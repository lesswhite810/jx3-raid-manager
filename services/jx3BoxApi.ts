import { db } from './db';
import { fetch } from '@tauri-apps/plugin-http';

export interface JX3Equip {
    ID: string; // 使用 API 原始的 id 字段（如 "8_41486"）
    Name: string;
    UiID: string;
    IconID: number | null;
    Level: number;
    Quality: string;
    TypeLabel?: string;
    SubTypeLabel?: string;
    attributes?: Array<{
        type: string;
        label: string;
        value?: string;
        [key: string]: any;
    }>;
    AttributeTypes?: Record<string, string>;
    [key: string]: any;
}

export interface JX3EquipResponse {
    total: number;
    per: number;
    pages: number;
    page: number;
    list: JX3Equip[];
}

export type EquipType = 'weapon' | 'armor' | 'trinket';

const CACHE_KEY = 'equip_cache_wuxiu';
const CACHE_DURATION = 24 * 60 * 60 * 1000; // 24 hours

// 同步状态追踪：用于让 getEquip 等待同步完成
let syncPromise: Promise<void> | null = null;

/**
 * 等待装备同步完成（如果正在同步）
 */
export async function waitForSync(): Promise<void> {
    if (syncPromise) {
        await syncPromise;
    }
}

/**
 * 同步装备数据到本地数据库
 * 每天只更新一次
 */
export async function syncEquipment(): Promise<void> {
    // 如果已经在同步中，等待现有同步完成
    if (syncPromise) {
        console.log('[Equipment Sync] 同步已在进行中，等待完成...');
        return syncPromise;
    }

    const doSync = async () => {
        try {
            const cache = await db.getCache(CACHE_KEY);
            const now = Date.now();

            // 检查数据库是否为空
            const existingEquips = await db.getEquipments();
            const isEmpty = !existingEquips || existingEquips.length === 0;

            if (!isEmpty && cache && (now - new Date(cache.updatedAt).getTime() < CACHE_DURATION)) {
                console.log('[Equipment Sync] 缓存有效，跳过同步');
                return;
            }

            if (isEmpty) {
                console.log('[Equipment Sync] 数据库为空，开始首次同步...');
            } else {
                console.log('[Equipment Sync] 缓存已过期，开始更新同步...');
            }

            console.log('[Equipment Sync] 正在获取装备数据，请稍候...');
            const startTime = Date.now();
            const allItems = await fetchAllEquipment('无修');
            const duration = ((Date.now() - startTime) / 1000).toFixed(1);

            if (allItems.length > 0) {
                // Save to new table
                console.log(`[Equipment Sync] 正在保存 ${allItems.length} 条装备数据到数据库...`);
                await db.saveEquipments(allItems);
                // Update cache timestamp marker (content is empty string now, just for time check)
                await db.saveCache(CACHE_KEY, 'synced');
                console.log(`[Equipment Sync] ✓ 同步完成！共同步 ${allItems.length} 件装备，耗时 ${duration}秒`);
            } else {
                console.log('[Equipment Sync] ⚠ 未获取到任何装备数据');
            }
        } catch (error) {
            console.error('[Equipment Sync] ✗ 同步失败:', error);
        } finally {
            syncPromise = null;
        }
    };

    syncPromise = doSync();
    return syncPromise;
}

/**
 * Helper to fetch all pages
 */
async function fetchAllEquipment(keyword: string): Promise<JX3Equip[]> {
    let allItems: JX3Equip[] = [];
    const per = 50;

    // 装备类别配置：auc_genre + auc_sub_type_id
    const equipCategories = [
        { auc_genre: 2, auc_sub_type_id: 1, name: '投掷' },
        { auc_genre: 3, auc_sub_type_id: 2, name: '帽子' },
        { auc_genre: 3, auc_sub_type_id: 5, name: '鞋子' },
        { auc_genre: 4, auc_sub_type_id: 1, name: '项链' },
        { auc_genre: 4, auc_sub_type_id: 3, name: '腰坠' },
    ];

    console.log(`[Equipment Sync] 准备获取 ${equipCategories.length} 个装备类别的数据`);

    try {
        // 并发获取所有装备类别
        const fetchCategory = async (category: typeof equipCategories[0]): Promise<JX3Equip[]> => {
            const categoryItems: JX3Equip[] = [];
            let page = 1;

            while (true) {
                const url = new URL('https://node.jx3box.com/api/node/item/search');
                url.searchParams.append('ids', ''); // 添加 ids 参数（空值），确保与 API 格式一致
                url.searchParams.append('keyword', keyword);
                url.searchParams.append('page', page.toString());
                url.searchParams.append('per', per.toString());
                url.searchParams.append('client', 'std');
                url.searchParams.append('auc_genre', category.auc_genre.toString());
                url.searchParams.append('auc_sub_type_id', category.auc_sub_type_id.toString());

                const response = await fetch(url.toString(), { method: 'GET' });
                if (!response.ok) break;

                const data = await response.json() as any;
                const list = data.data && data.data.data ? data.data.data : [];

                // 调试日志：记录每页获取的数据
                if (page === 1) {
                    console.log(`[Equipment Sync] 获取 ${category.name}: 第${page}页 ${list.length} 件`);
                }

                if (!list || list.length === 0) break;

                const mapped = list.map((item: any) => ({
                    ...item,
                    // 直接使用 API 的 id 字段作为主键（如 "8_41486"）
                    ID: item.id,
                    Name: item.Name || item.label,
                    UiID: item.UiID ? String(item.UiID) : '',
                    IconID: item.IconID,
                    Level: item.Level || item.level,
                    Quality: item.Quality ? String(item.Quality) : '',
                    TypeLabel: item.TypeLabel || '', // 装备类型标签（如：项链、腰坠等）
                    Recommend: item.Recommend || '', // Ensure it is a string
                    // 过滤掉不需要的属性类型
                    attributes: (item.attributes || []).filter((attr: any) =>
                        !['atRangeWeaponDamageBase', 'atRangeWeaponAttackSpeedBase'].includes(attr.type)
                    ),
                    AttributeTypes: item.AttributeTypes || {},
                    Diamonds: item.Diamonds || [],
                }));

                categoryItems.push(...mapped);

                if (mapped.length < per) break;
                if (page > 50) break;
                page++;

                await new Promise(r => setTimeout(r, 50)); // 减少延迟以提高效率
            }

            console.log(`[Equipment Sync] Category ${category.name} completed: ${categoryItems.length} items`);
            return categoryItems;
        };

        // 并发执行所有类别的获取
        console.log(`[Equipment Sync] Starting concurrent fetch for ${equipCategories.length} categories...`);
        const results = await Promise.all(equipCategories.map(cat => fetchCategory(cat)));

        // 合并所有结果
        allItems = results.flat();
        console.log(`[Equipment Sync] All categories completed, total items: ${allItems.length}`);

        // 调试日志：去重前后对比
        console.log(`[Equipment Sync] Before dedup: ${allItems.length} items`);

        // 使用 ID 字段进行去重（现在 ID = item.id，即 API 原始 id 如 "8_41486"）
        const uniqueItems = new Map<string, JX3Equip>();
        let duplicateCount = 0;
        allItems.forEach(item => {
            // 直接使用 ID 字段（字符串类型）
            if (item.ID && !uniqueItems.has(item.ID)) {
                uniqueItems.set(item.ID, item);
            } else {
                duplicateCount++;
            }
        });

        if (duplicateCount > 0) {
            console.log(`[Equipment Sync] 去重完成: ${uniqueItems.size} 件唯一装备，移除 ${duplicateCount} 件重复`);
        }

        // 注意：API 已经按关键词搜索，不再需要本地过滤
        // 移除 filterItem 调用，保留所有 API 返回的结果
        console.log(`[Equipment Sync] ✓ 共获取 ${uniqueItems.size} 件装备`);
        return Array.from(uniqueItems.values());

    } catch (e) {
        console.error('[Equipment Sync] ✗ 获取装备数据失败:', e);
        return allItems;
    }
}

/**
 * Filter item by keyword (supports multiple keywords separated by space)
 * Checks Name and Attributes
 */
function filterItem(item: JX3Equip, keyword: string): boolean {
    if (!keyword) return true;
    const tokens = keyword.split(/\s+/).filter(t => t.trim().length > 0);

    // Helper to get simplified attribute names
    const getAttrNames = (item: JX3Equip): string[] => {
        if (!item.attributes || !Array.isArray(item.attributes)) return [];
        return item.attributes.map(attr => {
            let name = item.AttributeTypes?.[attr.type];
            if (!name && attr.label) {
                const match = attr.label.match(/^([^\s0-9]+)/);
                if (match) name = match[1];
            }
            if (!name) return '';
            name = name.replace(/等级$|值$/, '');
            if (name === '外功攻击') return '外功';
            if (name === '内功攻击') return '内功';
            if (name === '会心效果') return '会效';
            if (name === '治疗成效') return '治疗';
            return name;
        }).filter(Boolean);
    };

    const attrNames = getAttrNames(item);

    // Each token must be found in Name OR Attributes
    return tokens.every(token => {
        // 1. Check Name
        if (item.Name && item.Name.includes(token)) return true;

        // 2. Check Attributes
        if (attrNames.some(attr => attr.includes(token))) return true;

        return false;
    });
}

/**
 * 获取指定装备数据
 * @param keyword ID 或 名称
 */
export async function getEquip(keyword: string | number): Promise<JX3Equip[]> {
    const isId = !isNaN(Number(keyword));

    // Try reading from cache first if searching for related items
    // Try reading from database first if searching for related items
    if (!isId) {
        try {
            // 首先检查数据库是否有数据
            let storedEquips = await db.getEquipments();

            // 如果数据库为空且正在同步，等待同步完成
            if ((!storedEquips || storedEquips.length === 0) && syncPromise) {
                console.log('[getEquip] Database empty, waiting for sync to complete...');
                await syncPromise;
                // 同步完成后重新获取
                storedEquips = await db.getEquipments();
            }

            if (storedEquips && storedEquips.length > 0) {
                const term = keyword.toString();
                // Filter locally
                const cachedResults = (storedEquips as JX3Equip[]).filter(item =>
                    filterItem(item, term)
                );

                if (cachedResults.length > 0) {
                    return cachedResults;
                }
            }
        } catch (e) {
            console.error('Error reading database:', e);
        }
    }

    try {
        const url = new URL('https://node.jx3box.com/api/node/item/search');
        // If keyword contains spaces, we might only send the first part to API 
        // OR send full string. API probably does fuzzy match on name only.
        // Better strategy: Send the first token to API to get candidates, then filter locally.
        // Assuming user types "无修 外功", finding "无修" items is better.
        const searchTerms = keyword.toString().split(/\s+/);
        const apiKeyword = searchTerms[0]; // Use first term for API search

        url.searchParams.append('keyword', apiKeyword);
        url.searchParams.append('page', '1');
        url.searchParams.append('per', '20'); // Maybe increase if filtering locally?
        url.searchParams.append('client', 'std');

        const response = await fetch(url.toString(), {
            method: 'GET',
        });

        if (!response.ok) {
            console.warn(`Failed to fetch items: ${response.status}`);
            return [];
        }

        const data = await response.json() as any;
        const list = data.data && data.data.data ? data.data.data : [];

        const result = list.map((item: any) => ({
            ...item,
            ID: item.SourceID || item.id,
            Name: item.Name || item.label,
            UiID: item.UiID ? String(item.UiID) : '',
            IconID: item.IconID,
            Level: item.Level || item.level,
            Quality: item.Quality ? String(item.Quality) : '',
            Recommend: item.Recommend || '',
            attributes: item.attributes || [],
            AttributeTypes: item.AttributeTypes || {},
            Diamonds: item.Diamonds || [],
        })).filter((item: JX3Equip) => {
            // Client-side filtering
            if (!isId && keyword) {
                return filterItem(item, keyword.toString());
            }
            return true;
        });

        // Deduplicate the single page result (just in case)
        const uniqueList = new Map<string, JX3Equip>();
        (result as JX3Equip[]).forEach(item => {
            if (item.ID && !uniqueList.has(item.ID)) {
                uniqueList.set(item.ID, item);
            }
        });

        return Array.from(uniqueList.values());

    } catch (error) {
        console.error(`Error fetching equipment:`, error);
        return [];
    }
}
