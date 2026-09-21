use anyhow::{Context, Result};
use sdk::applet::{AppletPriority, LocalizedString};
use std::fs;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct CargoToml {
    package: CargoPackage,
}

#[derive(serde::Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct AppletMetadata {
    name: LocalizedString,
    description: LocalizedString,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    priority: AppletPriority,
    #[serde(default)]
    category: LocalizedString,
}

fn normalize_and_validate_icon(icon: &str, applet_name: &str) -> Result<String> {
    if icon.trim().is_empty() {
        return Ok(String::new());
    }

    let rows: Vec<&str> = icon
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect();

    if rows.len() != 11 {
        anyhow::bail!(
            "❌ [{}] 아이콘 행(Row) 개수가 올바르지 않습니다.\n\
             - 필수 규격: 정확히 11개 행\n\
             - 현재 작성 상태: {}개 행",
            applet_name,
            rows.len()
        );
    }

    let mut normalized = String::new();
    for (idx, row) in rows.iter().enumerate() {
        let char_count = row.chars().count();
        if char_count != 11 {
            anyhow::bail!(
                "❌ [{}] 아이콘 {}번째 행의 가로 폭(Column)이 올바르지 않습니다.\n\
                 - 필수 규격: 정확히 11자\n\
                 - 현재 작성 상태: {}자 (\"{}\")",
                applet_name,
                idx + 1,
                char_count,
                row
            );
        }

        // Convert characters to '0' or '1'
        let mut row_normalized = String::new();
        for ch in row.chars() {
            match ch {
                '0' | ' ' | '.' | '░' => row_normalized.push('0'),
                '1' | '#' | '█' | '●' | '*' | 'x' | 'o' => row_normalized.push('1'),
                other => anyhow::bail!(
                    "❌ [{}] 아이콘 {}번째 행에 허용되지 않는 문자 '{}'가 포함되어 있습니다.\n\
                     - 허용 문자: 꺼짐('0', '.', ' ', '░') / 켜짐('1', '#', '█', '●', '*', 'x', 'o')",
                    applet_name,
                    idx + 1,
                    other
                ),
            }
        }
        normalized.push_str(&row_normalized);
        normalized.push('\n');
    }

    Ok(normalized)
}

fn create_applet_toml_template(applet_dir: &std::path::Path) -> Result<()> {
    let applet_toml_path = applet_dir.join("Applet.toml");
    if applet_toml_path.exists() {
        anyhow::bail!(
            "❌ Applet.toml이 이미 해당 경로에 존재합니다: {:?}",
            applet_toml_path
        );
    }

    // Ensure the parent directory exists
    if !applet_dir.exists() {
        fs::create_dir_all(applet_dir)
            .with_context(|| format!("Failed to create directory {:?}", applet_dir))?;
    }

    // Try to get applet name from Cargo.toml if it exists
    let cargo_path = applet_dir.join("Cargo.toml");
    let applet_name = if cargo_path.exists() {
        if let Ok(content) = fs::read_to_string(&cargo_path) {
            if let Ok(toml_val) = toml::from_str::<CargoToml>(&content) {
                toml_val.package.name
            } else {
                "new-applet".to_string()
            }
        } else {
            "new-applet".to_string()
        }
    } else {
        "new-applet".to_string()
    };

    let template = format!(
        r#"priority = "Normal"
hidden = false
icon = """
...#####...
.##.....##.
#.........#
#.........#
#.........#
#.........#
#.........#
#.........#
#.........#
.##.....##.
...#####...
"""

[category]
ko = "유틸리티"
en = "Utility"
ja = "ユーティリティ"

[name]
ko = "{}"
en = "{}"
ja = "{}"

[description]
ko = "새로운 애플릿 설명"
en = "New applet description"
ja = "新規アプリの説明"
"#,
        applet_name, applet_name, applet_name
    );

    fs::write(&applet_toml_path, template)
        .with_context(|| format!("Applet.toml 템플릿 생성 실패: {:?}", applet_toml_path))?;

    println!(
        "✨ Applet.toml 템플릿 파일이 생성되었습니다: {:?}",
        applet_toml_path
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        anyhow::bail!(
            "Usage: applet-metadata-tool <applet-dir-path> [--name | --version | --json | --init | --embed <input-wasm> -o <output-wasm>]"
        );
    }

    let applet_dir = PathBuf::from(&args[1]);

    // Parse arguments
    let mut input_wasm = None;
    let mut output_wasm = None;
    let mut mode = "--json";

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--name" => {
                mode = "--name";
                i += 1;
            }
            "--version" => {
                mode = "--version";
                i += 1;
            }
            "--json" => {
                mode = "--json";
                i += 1;
            }
            "--init" => {
                mode = "--init";
                i += 1;
            }
            "--embed" => {
                if i + 1 < args.len() {
                    input_wasm = Some(&args[i + 1]);
                    i += 2;
                } else {
                    anyhow::bail!("Missing value for --embed");
                }
            }
            "-o" => {
                if i + 1 < args.len() {
                    output_wasm = Some(&args[i + 1]);
                    i += 2;
                } else {
                    anyhow::bail!("Missing value for -o");
                }
            }
            other => {
                anyhow::bail!("Unknown option: {}", other);
            }
        }
    }

    // Execute --init if specified
    if mode == "--init" {
        return create_applet_toml_template(&applet_dir);
    }

    // Parse Cargo.toml
    let cargo_path = applet_dir.join("Cargo.toml");
    let cargo_content = fs::read_to_string(&cargo_path)
        .with_context(|| format!("Failed to read Cargo.toml at {:?}", cargo_path))?;
    let cargo_toml: CargoToml = toml::from_str(&cargo_content)
        .with_context(|| format!("Failed to parse Cargo.toml at {:?}", cargo_path))?;

    if mode == "--name" {
        println!("{}", cargo_toml.package.name);
        return Ok(());
    } else if mode == "--version" {
        println!("{}", cargo_toml.package.version);
        return Ok(());
    }

    // Parse Applet.toml
    let applet_toml_path = applet_dir.join("Applet.toml");
    let applet_toml_content = fs::read_to_string(&applet_toml_path)
        .with_context(|| format!("Failed to read Applet.toml at {:?}", applet_toml_path))?;
    let mut applet_metadata: AppletMetadata = toml::from_str(&applet_toml_content)
        .with_context(|| format!("Failed to parse Applet.toml at {:?}", applet_toml_path))?;

    // Normalize and strictly validate icon
    applet_metadata.icon =
        normalize_and_validate_icon(&applet_metadata.icon, &cargo_toml.package.name)?;

    if let Some(input) = input_wasm {
        let output = output_wasm.context("Missing output file (-o)")?;
        let json_str = serde_json::to_string(&applet_metadata)
            .context("Failed to serialize applet metadata to JSON")?;

        let status = std::process::Command::new("wasm-tools")
            .arg("metadata")
            .arg("add")
            .arg("--name")
            .arg(&cargo_toml.package.name)
            .arg("--version")
            .arg(&cargo_toml.package.version)
            .arg("--description")
            .arg(&json_str)
            .arg(input)
            .arg("-o")
            .arg(output)
            .status()
            .context("Failed to execute wasm-tools. Make sure wasm-tools is installed.")?;

        if !status.success() {
            anyhow::bail!("wasm-tools execution failed");
        }
        return Ok(());
    }

    if mode == "--json" {
        let json_str = serde_json::to_string(&applet_metadata)
            .context("Failed to serialize applet metadata to JSON")?;
        println!("{}", json_str);
    } else {
        anyhow::bail!("Unknown mode: {}", mode);
    }

    Ok(())
}
