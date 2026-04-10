use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, Deserialize)]
pub struct Force {
    pub id: i32,
    pub name: String,
    pub kungfus: Vec<Kungfu>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kungfu {
    #[allow(dead_code)]
    pub id: i32,
    pub name: String,
    // 前端展示配置字段（后端当前仅用于数据序列化一致性）
    #[allow(dead_code)]
    pub icon_id: i32,
    #[allow(dead_code)]
    pub color: String,
    #[allow(dead_code)]
    pub text_color: String,
    #[allow(dead_code)]
    pub border_color: String,
    #[allow(dead_code)]
    pub short_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KungfuData {
    pub forces: Vec<Force>,
}

static KUNGFU_DATA: LazyLock<KungfuData> = LazyLock::new(|| {
    let json_str = include_str!("../../data/kungfu_data.json");
    serde_json::from_str(json_str).expect("Failed to parse kungfu_data.json")
});

/// 获取心法数据
#[allow(dead_code)]
pub fn get_kungfu_data() -> &'static KungfuData {
    &KUNGFU_DATA
}

#[allow(dead_code)]
static FORCE_ID_TO_NAME: LazyLock<HashMap<i32, String>> = LazyLock::new(|| {
    KUNGFU_DATA
        .forces
        .iter()
        .map(|f| (f.id, f.name.clone()))
        .collect()
});

static FORCE_NAME_TO_ID: LazyLock<HashMap<String, i32>> = LazyLock::new(|| {
    KUNGFU_DATA
        .forces
        .iter()
        .map(|f| (f.name.clone(), f.id))
        .collect()
});

static MARTIAL_TO_SECT: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    KUNGFU_DATA
        .forces
        .iter()
        .flat_map(|force| {
            force
                .kungfus
                .iter()
                .map(move |k| (k.name.clone(), force.name.clone()))
        })
        .collect()
});

#[allow(dead_code)]
static KUNGFU_FORCE_NAME_TO_ID: LazyLock<HashMap<(i32, String), i32>> = LazyLock::new(|| {
    KUNGFU_DATA
        .forces
        .iter()
        .flat_map(|force| {
            force
                .kungfus
                .iter()
                .map(move |k| ((force.id, k.name.clone()), k.id))
        })
        .collect()
});

#[allow(dead_code)]
pub fn get_force_name(force_id: i32) -> Option<String> {
    FORCE_ID_TO_NAME.get(&force_id).cloned()
}

pub fn get_force_id_by_name(force_name: &str) -> Option<i32> {
    FORCE_NAME_TO_ID.get(force_name).copied()
}

pub fn get_sect_by_martial(martial: &str) -> Option<&'static str> {
    MARTIAL_TO_SECT.get(martial).map(|s| s.as_str())
}

#[allow(dead_code)]
pub fn build_kungfu_force_name_to_id_map() -> HashMap<(i32, String), i32> {
    KUNGFU_FORCE_NAME_TO_ID.clone()
}

/// 获取心法门派到ID映射的静态引用（避免重复clone）
pub fn get_kungfu_force_name_to_id_map() -> &'static HashMap<(i32, String), i32> {
    &KUNGFU_FORCE_NAME_TO_ID
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_force_name() {
        assert_eq!(get_force_name(0), Some("无相楼".to_string()));
        assert_eq!(get_force_name(1), Some("少林".to_string()));
        assert_eq!(get_force_name(214), Some("万灵".to_string()));
        assert_eq!(get_force_name(999), None);
    }

    #[test]
    fn test_get_force_id_by_name() {
        assert_eq!(get_force_id_by_name("万灵"), Some(214));
        assert_eq!(get_force_id_by_name("不存在"), None);
    }

    #[test]
    fn test_get_sect_by_martial() {
        assert_eq!(get_sect_by_martial("山海心诀"), Some("万灵"));
        assert_eq!(get_sect_by_martial("洗髓经"), Some("少林"));
        assert_eq!(get_sect_by_martial("不存在"), None);
    }

    #[test]
    fn test_build_kungfu_map() {
        let map = build_kungfu_force_name_to_id_map();
        assert_eq!(map.get(&(214, "山海心诀".to_string())), Some(&10756));
        assert_eq!(map.get(&(8, "问水诀".to_string())), Some(&10144));
        assert_eq!(map.get(&(8, "山居剑意".to_string())), Some(&10145));
    }
}
