//! USB 시리얼(CDC ACM 가젯)로 연결된 기기에 네트워크 정보를 질의하는 CLI 도구입니다.
//!
//! `capture` GUI 앱의 Wi-Fi 패널과 같은 프로토콜(`protocols::capture`)을 쓰지만,
//! `just deploy-*` 에 필요한 기기 IP 만 빠르게 알아내는 용도로 만든 최소 기능 버전입니다.
//!
//! ```text
//! cargo run -p device-info                       # 기기 IP 출력
//! cargo run -p device-info -- scan               # 주변 Wi-Fi 스캔
//! cargo run -p device-info -- wifi <SSID> <PW>   # Wi-Fi 연결 후 IP 출력
//! ```

use std::net::Ipv4Addr;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use host_utils::serial_comm::{SerialComm, available_ports};
use protocols::capture::{CaptureRequestToRuntime, CaptureResponseFromRuntime};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::timeout;

/// 기기가 노출하는 CDC ACM 가젯의 USB VID/PID (`pc-apps/capture/src/app.rs` 와 동일)
const GADGET_VID: u16 = 0x0525;
const GADGET_PID: u16 = 0xA4A7;
/// capture 앱이 쓰는 기본 보율
const BAUD_RATE: u32 = 460_800;

#[derive(Parser)]
#[command(
    about = "USB 로 연결된 점자 디스플레이 기기의 네트워크 정보를 조회합니다.",
    long_about = None
)]
struct Args {
    /// 사용할 시리얼 포트. 생략하면 USB VID/PID 로 자동 탐색합니다.
    #[arg(long)]
    port: Option<String>,

    /// 연결 및 응답 대기 시간(초)
    #[arg(long, default_value_t = 20)]
    timeout: u64,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// 기기의 IP 주소를 출력합니다. (기본 동작)
    Ip,
    /// 기기 주변의 Wi-Fi 목록을 스캔합니다.
    Scan,
    /// 기기를 Wi-Fi 에 연결한 뒤 IP 를 출력합니다.
    Wifi {
        ssid: String,
        password: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let wait = Duration::from_secs(args.timeout);

    let port = resolve_port(args.port)?;
    println!("포트 {port} 로 연결하는 중...");

    let SerialComm {
        tx_sender,
        mut rx_receiver,
        mut status_receiver,
    } = SerialComm::<CaptureRequestToRuntime, CaptureResponseFromRuntime>::new(
        &port,
        BAUD_RATE,
        Some((GADGET_VID, GADGET_PID)),
    )
    .context("시리얼 포트를 열지 못했습니다")?;

    timeout(wait, async {
        while let Some(status) = status_receiver.recv().await {
            if status.is_connected() {
                return Ok::<(), anyhow::Error>(());
            }
        }
        bail!("연결 상태 채널이 닫혔습니다")
    })
    .await
    .map_err(|_| {
        anyhow!(
            "{port} 연결이 {}초 안에 이뤄지지 않았습니다.\n\
             기기가 켜져 있고 runtime-native 서비스가 돌고 있는지 확인하세요.",
            args.timeout
        )
    })??;

    println!("연결됨.\n");

    match args.command.unwrap_or(Command::Ip) {
        Command::Ip => {
            let ips = fetch_addresses(&tx_sender, &mut rx_receiver, wait).await?;
            print_addresses(&ips);
        }
        Command::Scan => {
            let res = ask(
                &tx_sender,
                &mut rx_receiver,
                CaptureRequestToRuntime::ScanWifi,
                wait,
                |r| matches!(r, CaptureResponseFromRuntime::WifiScanResult(_)),
            )
            .await?;
            let CaptureResponseFromRuntime::WifiScanResult(mut networks) = res else {
                unreachable!()
            };
            if networks.is_empty() {
                println!("검색된 Wi-Fi 가 없습니다.");
            } else {
                networks.sort_by_key(|n| -n.signal);
                println!("{:<32} {:>6}  {}", "SSID", "신호", "보안");
                println!("{}", "-".repeat(56));
                for n in networks {
                    println!("{:<32} {:>5}%  {}", n.ssid, n.signal, n.security);
                }
            }
        }
        Command::Wifi { ssid, password } => {
            println!("'{ssid}' 에 연결하는 중...");
            let res = ask(
                &tx_sender,
                &mut rx_receiver,
                CaptureRequestToRuntime::ConnectWifi {
                    ssid: ssid.clone(),
                    passphrase: password,
                },
                wait,
                |r| matches!(r, CaptureResponseFromRuntime::WifiConnectResult { .. }),
            )
            .await?;
            let CaptureResponseFromRuntime::WifiConnectResult { success, message } = res else {
                unreachable!()
            };
            if !success {
                bail!("Wi-Fi 연결 실패: {message}");
            }
            println!("연결 성공: {message}\n");

            let ips = fetch_addresses(&tx_sender, &mut rx_receiver, wait).await?;
            print_addresses(&ips);
        }
    }

    Ok(())
}

/// `--port` 가 없으면 가젯 VID/PID 로 포트를 자동 탐색합니다.
fn resolve_port(explicit: Option<String>) -> Result<String> {
    if let Some(port) = explicit {
        return Ok(port);
    }

    let ports = available_ports(Some((GADGET_VID, GADGET_PID)));
    match ports.len() {
        0 => bail!(
            "USB 로 연결된 기기를 찾지 못했습니다.\n\
             - 기기가 켜져 있는지\n\
             - 케이블이 데이터 전송용인지 (충전 전용 아님)\n\
             - CM5 의 USB-C(OTG) 포트에 꽂혀 있는지\n\
             확인한 뒤 다시 시도하거나, --port COM3 처럼 직접 지정하세요."
        ),
        1 => Ok(ports[0].clone()),
        _ => {
            println!("여러 포트가 검색되어 첫 번째를 사용합니다: {ports:?}");
            Ok(ports[0].clone())
        }
    }
}

async fn fetch_addresses(
    tx: &Sender<CaptureRequestToRuntime>,
    rx: &mut Receiver<CaptureResponseFromRuntime>,
    wait: Duration,
) -> Result<Vec<String>> {
    let res = ask(
        tx,
        rx,
        CaptureRequestToRuntime::GetNetworkAddresses,
        wait,
        |r| matches!(r, CaptureResponseFromRuntime::NetworkAddressesResult(_)),
    )
    .await?;
    let CaptureResponseFromRuntime::NetworkAddressesResult(ips) = res else {
        unreachable!()
    };
    Ok(ips)
}

/// 요청을 보내고, 조건에 맞는 첫 응답을 기다립니다.
/// 런타임이 보내는 다른 응답(`Ok`, `Paused` 등)은 건너뜁니다.
async fn ask(
    tx: &Sender<CaptureRequestToRuntime>,
    rx: &mut Receiver<CaptureResponseFromRuntime>,
    req: CaptureRequestToRuntime,
    wait: Duration,
    accept: impl Fn(&CaptureResponseFromRuntime) -> bool,
) -> Result<CaptureResponseFromRuntime> {
    tx.send(req)
        .await
        .map_err(|_| anyhow!("요청을 전송하지 못했습니다 (연결이 끊어졌습니다)"))?;

    timeout(wait, async {
        while let Some(res) = rx.recv().await {
            if accept(&res) {
                return Ok(res);
            }
        }
        bail!("응답 채널이 닫혔습니다")
    })
    .await
    .map_err(|_| anyhow!("기기가 시간 안에 응답하지 않았습니다"))?
}

fn print_addresses(ips: &[String]) {
    if ips.is_empty() {
        println!("기기에 할당된 IP 가 없습니다. Wi-Fi 나 이더넷에 연결되어 있지 않습니다.");
        println!("  cargo run -p device-info -- scan");
        println!("  cargo run -p device-info -- wifi <SSID> <비밀번호>");
        return;
    }

    println!("기기의 IP 주소:");
    for ip in ips {
        println!("  {ip}{}", if is_usable(ip) { "" } else { "   (배포에 쓸 수 없음)" });
    }

    if let Some(best) = ips.iter().find(|ip| is_usable(ip)) {
        println!("\n이 주소로 배포하려면:");
        println!("  echo 'export DEVICE_HOST=muon@{best}' > device-env.sh");
        println!("  just deploy-applets");
    } else {
        println!("\n일반 네트워크 주소가 없습니다. Wi-Fi 또는 랜선 연결이 필요합니다.");
    }
}

/// 링크로컬(169.254.x)·루프백·IPv6 를 걸러 실제로 scp 가 가능한 주소만 고릅니다.
fn is_usable(ip: &str) -> bool {
    match ip.parse::<Ipv4Addr>() {
        Ok(v4) => !v4.is_loopback() && !v4.is_link_local() && !v4.is_unspecified(),
        Err(_) => false,
    }
}
