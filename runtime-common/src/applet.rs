use sdk::applet::{AppletInfo, AppletPriority, LocalizedString};
use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct AppletMetadata {
    pub name: LocalizedString,
    pub description: LocalizedString,
    pub icon: String,
    pub hidden: bool,
    pub priority: AppletPriority,
    pub category: LocalizedString,
}


/// WASM 바이너리에서 AppletInfo 정보를 추출합니다.
pub fn parse_applet_info(name: &str, wasm_bytes: &[u8]) -> AppletInfo {
    let mut version = String::new();
    let mut description = LocalizedString::default();
    let mut icon = String::new();
    let mut hidden = false;
    let mut priority = AppletPriority::Low;
    let mut category = LocalizedString::default();
    let mut localized_name = LocalizedString::default();


    if let Ok(payload) = wasm_metadata::Payload::from_binary(wasm_bytes) {
        let metadata = match &payload {
            wasm_metadata::Payload::Component { metadata, .. } => metadata,
            wasm_metadata::Payload::Module(metadata) => metadata,
        };

        if let Some(desc) = &metadata.description
            && let Ok(parsed) = serde_json::from_str::<AppletMetadata>(&desc.to_string())
        {
            localized_name = parsed.name;
            description = parsed.description;
            icon = parsed.icon;
            hidden = parsed.hidden;
            priority = parsed.priority;
            category = parsed.category;
        }
        if let Some(ver) = &metadata.version {
            version = ver.to_string();
        }
    }

    // 메타데이터가 없거나 비어있는 경우, 앱의 내부 이름을 기본값으로 사용합니다.
    if localized_name.ko.is_empty() && localized_name.en.is_empty() && localized_name.ja.is_empty()
    {
        localized_name.en = name.to_string();
    }
    if description.ko.is_empty() && description.en.is_empty() && description.ja.is_empty() {
        description.en = name.to_string();
    }

    AppletInfo {
        id: name.to_string(),
        name: localized_name,
        version,
        description,
        icon,
        hidden,
        priority,
        category,
    }
}
