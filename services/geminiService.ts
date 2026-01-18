import { RaidRecord, Account } from "../types";

export const analyzeRaidPerformance = async (
  records: RaidRecord[], 
  _accounts: Account[]
): Promise<string> => {
  // 延迟加载GoogleGenAI，避免在Tauri环境中模块加载时崩溃
  try {
    // 检查是否是Tauri环境
    const isTauri = typeof window !== 'undefined' && window.__tauri__ !== undefined;
    
    if (isTauri) {
      return "AI分析功能仅在浏览器环境中可用。";
    }
    
    // 仅在非Tauri环境中动态导入GoogleGenAI
    const { GoogleGenAI } = await import("@google/genai");
    
    // Safely access environment variables
    let apiKey = '';
    try {
      // @ts-ignore
      if (typeof import.meta !== 'undefined' && import.meta.env) {
        // @ts-ignore
        apiKey = import.meta.env.VITE_API_KEY || '';
      }
    } catch (e) {
      console.warn("Failed to access import.meta.env");
    }
    
    if (!apiKey) return "API Key not configured. Cannot generate analysis.";

    const ai = new GoogleGenAI({ apiKey });
    const recentRecords = records.slice(0, 50); // Analyze last 50 records to save tokens
    
    // Serialize data for the prompt
    const dataSummary = JSON.stringify({
      totalRuns: recentRecords.length,
      totalGold: recentRecords.reduce((acc, r) => acc + r.goldIncome, 0),
      xuanjingDrops: recentRecords.filter(r => r.hasXuanjing).length,
      clearedCount: recentRecords.filter(r => r.isCleared).length,
      recentLog: recentRecords.map(r => ({
        raid: r.raidName,
        gold: r.goldIncome,
        xuanjing: r.hasXuanjing
      }))
    });

    const prompt = `
      Analyze this JX3 (剑网三) raid data for a player who manages multiple accounts (including boosting/client accounts).
      Data: ${dataSummary}
      
      Please provide a concise, witty weekly summary (max 150 words) in Chinese.
      1. Comment on the "Xuanjing" (玄晶 - Big Rare Drop) luck.
      2. Evaluate the gold income efficiency.
      3. If there are client accounts, mention if the "Gongzhe" (working hard) vibe is strong.
      4. Use gaming terminology relevant to JX3.
    `;

    const response = await ai.models.generateContent({
      model: 'gemini-3-flash-preview',
      contents: prompt,
    });
    return response.text || "无法生成分析报告。";
  } catch (error) {
    console.error("AI Analysis Error:", error);
    return "分析服务暂时不可用，请稍后重试。";
  }
};