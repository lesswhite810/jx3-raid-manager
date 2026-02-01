import { db } from './db';
import { fetch } from '@tauri-apps/api/http';

export interface JX3Equip {
    ID: number;
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

/**
 * 同步装备数据到本地数据库
 * 每天只更新一次
 */
export async function syncEquipment(): Promise<void> {
    try {
        const cache = await db.getCache(CACHE_KEY);
        const now = Date.now();

        if (cache && (now - new Date(cache.updatedAt).getTime() < CACHE_DURATION)) {
            console.log('Equipment cache is valid, skipping sync.');
            return;
        }

        console.log('Starting equipment sync...');
        const allItems = await fetchAllEquipment('无修');
        if (allItems.length > 0) {
            await db.saveCache(CACHE_KEY, allItems);
            console.log(`Synced ${allItems.length} equipment items to cache.`);
        }
    } catch (error) {
        console.error('Failed to sync equipment:', error);
    }
}

/**
 * Helper to fetch all pages
 */
async function fetchAllEquipment(keyword: string): Promise<JX3Equip[]> {
    let allItems: JX3Equip[] = [];
    let page = 1;
    const per = 50;

    try {
        while (true) {
            const url = new URL('https://node.jx3box.com/api/node/item/search');
            url.searchParams.append('keyword', keyword);
            url.searchParams.append('page', page.toString());
            url.searchParams.append('per', per.toString());
            url.searchParams.append('client', 'std');

            const response = await fetch(url.toString(), { method: 'GET' });
            if (!response.ok) break;

            const data = response.data as any;
            const list = data.data && data.data.data ? data.data.data : [];

            if (!list || list.length === 0) break;

            const mapped = list.map((item: any) => ({
                ID: item.SourceID || item.id,
                Name: item.Name || item.label,
                UiID: item.UiID ? item.UiID.toString() : '',
                IconID: item.IconID,
                Level: item.Level || item.level,
                Quality: item.Quality,
                ...item
            }));

            allItems = allItems.concat(mapped);

            if (mapped.length < per) break;
            if (page > 50) break;
            page++;

            await new Promise(r => setTimeout(r, 100));
        }

        // Deduplicate by ID
        const uniqueItems = new Map<number, JX3Equip>();
        allItems.forEach(item => {
            if (item.ID && !uniqueItems.has(item.ID)) {
                uniqueItems.set(item.ID, item);
            }
        });

        return Array.from(uniqueItems.values()).filter(item => filterItem(item, keyword));

    } catch (e) {
        console.error('Error fetching all equipment:', e);
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
    if (!isId) {
        try {
            const cache = await db.getCache(CACHE_KEY);
            if (cache && cache.value && Array.isArray(cache.value)) {
                const term = keyword.toString();
                // Filter locally with new logic
                const cachedResults = (cache.value as JX3Equip[]).filter(item =>
                    filterItem(item, term)
                );

                if (cachedResults.length > 0) {
                    return cachedResults;
                }
            }
        } catch (e) {
            console.error('Error reading cache:', e);
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

        const data = response.data as any;
        const list = data.data && data.data.data ? data.data.data : [];

        const result = list.map((item: any) => ({
            ID: item.SourceID || item.id,
            Name: item.Name || item.label,
            UiID: item.UiID ? item.UiID.toString() : '',
            IconID: item.IconID,
            Level: item.Level || item.level,
            Quality: item.Quality,
            ...item
        })).filter((item: JX3Equip) => {
            // Client-side filtering
            if (!isId && keyword) {
                return filterItem(item, keyword.toString());
            }
            return true;
        });

        // Deduplicate the single page result (just in case)
        const uniqueList = new Map<number, JX3Equip>();
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
