use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct GkpParseRequest {
    pub buffer: Vec<u8>,
    pub options: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GkpParseResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[tauri::command]
pub fn parse_binary_gkp(request: GkpParseRequest) -> Result<GkpParseResponse, String> {
    // 简单的二进制检测
    if request.buffer.len() < 8 {
        return Ok(GkpParseResponse {
            success: false,
            data: None,
            error: Some("文件太小".to_string()),
        });
    }

    // 检查是否是二进制格式
    let mut is_binary = false;
    if request.buffer.len() >= 8 {
        // 检查二进制标识
        let mut sig = [0u8; 8];
        for i in 0..8 {
            if i < request.buffer.len() {
                sig[i] = request.buffer[i];
            }
        }
        
        // 检查常见二进制标识
        if sig == [0x20, 0x61, 0x74, 0x17, 0x46, 0x14, 0x46, 0x17] { // " aF\x14F\x17"
            is_binary = true;
        }
    }

    if is_binary {
        // 尝试解析为Lua表
        match parse_lua_from_binary(&request.buffer) {
            Ok(data) => {
                return Ok(GkpParseResponse {
                    success: true,
                    data: Some(data),
                    error: None,
                });
            }
            Err(e) => {
                return Ok(GkpParseResponse {
                    success: false,
                    data: None,
                    error: Some(format!("解析失败: {}", e)),
                });
            }
        }
    } else {
        // 尝试解析为文本
        match String::from_utf8(request.buffer.clone()) {
            Ok(text) => {
                // 简单检查是否是Lua表
                if text.trim().starts_with('{') && text.trim().ends_with('}') {
                    match serde_json::from_str(&text) {
                        Ok(data) => {
                            return Ok(GkpParseResponse {
                                success: true,
                                data: Some(data),
                                error: None,
                            });
                        }
                        Err(_) => {
                            return Ok(GkpParseResponse {
                                success: false,
                                data: None,
                                error: Some("无法解析JSON".to_string()),
                            });
                        }
                    }
                } else {
                    return Ok(GkpParseResponse {
                        success: false,
                        data: None,
                        error: Some("不是Lua表格式".to_string()),
                    });
                }
            }
            Err(_) => {
                return Ok(GkpParseResponse {
                    success: false,
                    data: None,
                    error: Some("UTF-8解码失败".to_string()),
                });
            }
        }
    }
}

// 简单的Lua解析函数
fn parse_lua_from_binary(buffer: &[u8]) -> Result<serde_json::Value, String> {
    let text = String::from_utf8(buffer.to_vec()).map_err(|_| "UTF-8解码失败".to_string())?;
    
    // 简单的Lua表解析
    if !text.trim().starts_with('{') || !text.trim().ends_with('}') {
        return Err("不是Lua表格式".to_string());
    }
    
    // 尝试解析为JSON
    serde_json::from_str(&text).map_err(|e| format!("JSON解析失败: {}", e))
}