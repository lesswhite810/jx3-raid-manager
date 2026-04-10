use crate::kungfu_data::{get_force_name, get_force_id_by_name, get_sect_by_martial, build_kungfu_map};

#[test]
fn test_wanling() {
    // 测试 214 -> 万灵
    let name = get_force_name(214);
    println!("get_force_name(214) = {:?}", name);
    assert_eq!(name, Some("万灵".to_string()));
    
    // 测试 万灵 -> 214
    let id = get_force_id_by_name("万灵");
    println!("get_force_id_by_name(\"万灵\") = {:?}", id);
    assert_eq!(id, Some(214));
    
    // 测试 山海心诀 -> 万灵
    let sect = get_sect_by_martial("山海心诀");
    println!("get_sect_by_martial(\"山海心诀\") = {:?}", sect);
    assert_eq!(sect, Some("万灵".to_string()));
    
    // 测试 (214, 山海心诀) -> 10756
    let map = build_kungfu_map();
    let kungfu_id = map.get(&(214, "山海心诀".to_string()));
    println!("kungfu_map.get(&(214, \"山海心诀\")) = {:?}", kungfu_id);
    assert_eq!(kungfu_id, Some(&10756));
}
