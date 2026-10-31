import { db } from './db';
import { fetch } from '@tauri-apps/plugin-http';

export interface JX3Equip {
    ID: number;
    Name: string;
    UiID: string;
    IconID: number | null;
    Level: number;
    Quality: string;
    SubType: number;
    TypeLabel?: string;
    BindType?: number;
    _IconID?: number;
    _Attrs?: string[];
    _AttrType?: string[];
    _Duty?: number;
    _PvType?: number;
    attributes?: Array<{
        type: string;
        label: string;
        value?: string;
        [key: string]: unknown;
    }>;
    AttributeTypes?: Record<string, string>;
    [key: string]: unknown;
}

// 装备 Magic 属性字段的运行时形态：
// - 字符串形态：直接是属性类型标识（如 'atSkillEventHandler'）
// - 对象形态：{ attr: string[] } 包含属性类型数组
export type JX3EquipMagicType = string | { attr?: string[] };

export interface JX3EquipResponse {
    total: number;
    per: number;
    pages: number;
    page: number;
    list: JX3Equip[];
}

export type EquipType = 'weapon' | 'armor' | 'trinket';

export interface GetEquipOptions {
    /**
     * 当前赛季刚确认品级边界时，忽略尚未被后台同步刷新过的本地装备缓存。
     * 远端失败或空结果时由调用方提示重试，绝不混回旧赛季缓存。
     */
    preferRemote?: boolean;
    /** 仅在 preferRemote 时传入已确认的边界；未确认的一端不参与请求。 */
    minLevel?: number;
    maxLevel?: number;
}

const REMOTE_EQUIP_TYPES: EquipType[] = ['armor', 'trinket', 'weapon'];
const REMOTE_PAGE_SIZE = 100;
const MAX_REMOTE_PAGES = 10;

const ATTR_NAME_MAP: Record<string, string> = {
    'atVitalityBase': '体质',
    'atSpiritBase': '根骨',
    'atStrengthBase': '力道',
    'atAgilityBase': '身法',
    'atSpunkBase': '元气',
    'atPhysicsAttackPowerBase': '外功',
    'atMagicAttackPowerBase': '内功',
    'atSolarAttackPowerBase': '内功',
    'atLunarAttackPowerBase': '内功',
    'atPoisonAttackPowerBase': '内功',
    'atNeutralAttackPowerBase': '内功',
    'atPhysicsCriticalStrike': '会心',
    'atMagicCriticalStrike': '会心',
    'atAllTypeCriticalStrike': '会心',
    'atPhysicsCriticalDamagePowerBase': '会效',
    'atMagicCriticalDamagePowerBase': '会效',
    'atAllTypeCriticalDamagePowerBase': '会效',
    'atPhysicsOvercome': '破防',
    'atPhysicsOvercomeBase': '破防',
    'atMagicOvercome': '破防',
    'atMagicOvercomeBase': '破防',
    'atSolarOvercomeBase': '破防',
    'atLunarOvercomeBase': '破防',
    'atPoisonOvercomeBase': '破防',
    'atNeutralOvercomeBase': '破防',
    'atSurplusValueBase': '破招',
    'atStrainBase': '无双',
    'atHaste': '加速',
    'atToughnessBase': '御劲',
    'atDecriticalDamagePowerBase': '化劲',
    'atPhysicsShieldBase': '外防',
    'atMagicShield': '内防',
    'atDodge': '闪避',
    'atParry': '招架',
    'atHit': '命中',
    'atTherapyPowerBase': '治疗',
};

function filterItem(item: JX3Equip, keyword: string): boolean {
    if (!keyword) return true;
    const tokens = keyword.split(/\s+/).filter(t => t.trim().length > 0);

    const getAttrNames = (item: JX3Equip): string[] => {
        const names: string[] = [];
        
        if (item._Attrs && Array.isArray(item._Attrs)) {
            const attrMap: Record<string, string> = {
                'Critical': '会心',
                'CriticalDamage': '会效',
                'Overcome': '破防',
                'Surplus': '破招',
                'Strain': '无双',
                'Haste': '加速',
                'Toughness': '御劲',
                'Decritical': '化劲',
                'PhysicsShield': '外防',
                'MagicShield': '内防',
                'Dodge': '闪避',
                'Parry': '招架',
                'Hit': '命中',
                'Therapy': '治疗',
            };
            item._Attrs.forEach((attr: string) => {
                if (attrMap[attr]) names.push(attrMap[attr]);
            });
        }

        if (item._AttrType && Array.isArray(item._AttrType)) {
            item._AttrType.forEach((attr: string) => {
                if (ATTR_NAME_MAP[attr]) names.push(ATTR_NAME_MAP[attr]);
            });
        }

        if (item.attributes && Array.isArray(item.attributes)) {
            item.attributes.forEach(attr => {
                let name = item.AttributeTypes?.[attr.type];
                if (!name && attr.label) {
                    const match = attr.label.match(/^([^\s0-9]+)/);
                    if (match) name = match[1];
                }
                if (name) {
                    name = name.replace(/等级$|值$/, '');
                    if (name === '外功攻击') name = '外功';
                    if (name === '内功攻击') name = '内功';
                    if (name === '会心效果') name = '会效';
                    if (name === '治疗成效') name = '治疗';
                    names.push(name);
                }
            });
        }

        return [...new Set(names)];
    };

    const attrNames = getAttrNames(item);

    return tokens.every(token => {
        if (item.Name && item.Name.includes(token)) return true;
        if (attrNames.some(attr => attr.includes(token))) return true;
        return false;
    });
}

const appendConfiguredLevelBounds = (url: URL, options: GetEquipOptions): void => {
    if (typeof options.minLevel === 'number' && Number.isInteger(options.minLevel) && options.minLevel > 0) {
        url.searchParams.append('min_level', String(options.minLevel));
    }
    if (typeof options.maxLevel === 'number' && Number.isInteger(options.maxLevel) && options.maxLevel > 0) {
        url.searchParams.append('max_level', String(options.maxLevel));
    }
};

const getEquipmentTypeLabel = (item: JX3Equip): string => {
    const detailType = Number(item.DetailType ?? 0);
    if (detailType === 6) return '暗器';

    switch (Number(item.SubType ?? 0)) {
        case 1: return '武器';
        case 2: return '上衣';
        case 3: return '帽子';
        case 4: return '项链';
        case 5: return '戒指';
        case 6: return '腰带';
        case 7: return '腰坠';
        case 8: return '下装';
        case 9: return '鞋子';
        case 10: return '护腕';
        default: return '其他';
    }
};

const normalizeRemoteEquipment = (item: JX3Equip): JX3Equip => ({
    ...item,
    TypeLabel: item.TypeLabel || getEquipmentTypeLabel(item),
});

const fetchRemoteEquipmentPage = async (
    equipType: EquipType,
    keyword: string | number,
    page: number,
    per: number,
    options: GetEquipOptions
): Promise<JX3EquipResponse | null> => {
    try {
        const url = new URL(`https://node.jx3box.com/equip/${equipType}`);
        url.searchParams.append('name', keyword.toString());
        url.searchParams.append('client', 'std');
        url.searchParams.append('page', String(page));
        url.searchParams.append('per', String(per));
        appendConfiguredLevelBounds(url, options);

        const response = await fetch(url.toString(), { method: 'GET' });
        if (!response.ok) {
            console.warn(`Failed to fetch ${equipType} page ${page}: ${response.status}`);
            return null;
        }
        return await response.json() as JX3EquipResponse;
    } catch (error) {
        console.error(`Error fetching ${equipType} page ${page}:`, error);
        return null;
    }
};

const fetchAllRemoteEquipments = async (
    keyword: string | number,
    options: GetEquipOptions
): Promise<JX3Equip[]> => {
    const fetchOneType = async (equipType: EquipType): Promise<JX3Equip[] | null> => {
        const firstPage = await fetchRemoteEquipmentPage(
            equipType, keyword, 1, REMOTE_PAGE_SIZE, options
        );
        if (!firstPage) return null;

        const pageCount = Number.isInteger(firstPage.pages) && firstPage.pages > 0
            ? firstPage.pages
            : 1;
        if (pageCount > MAX_REMOTE_PAGES) {
            console.warn(`Remote ${equipType} has ${pageCount} pages, exceeding supported cap ${MAX_REMOTE_PAGES}`);
            return null;
        }
        const remainingPages = await Promise.all(
            Array.from({ length: pageCount - 1 }, (_, index) =>
                fetchRemoteEquipmentPage(equipType, keyword, index + 2, REMOTE_PAGE_SIZE, options)
            )
        );
        if (remainingPages.some(page => page === null)) return null;

        return [
            ...(firstPage.list || []),
            ...remainingPages.flatMap(page => page?.list || []),
        ].map(normalizeRemoteEquipment);
    };

    const results = await Promise.all(REMOTE_EQUIP_TYPES.map(fetchOneType));
    // 任一类别或分页失败时不能把部分结果当作完整装备清单交给选择器。
    if (results.some(result => result === null)) return [];
    return results.flatMap(result => result ?? []);
};

/**
 * 获取指定装备数据
 */
export async function getEquip(
    keyword: string | number,
    options: GetEquipOptions = {}
): Promise<JX3Equip[]> {
    const isId = !isNaN(Number(keyword));

    if (options.preferRemote) {
        return fetchAllRemoteEquipments(keyword, options);
    }

    if (!isId) {
        try {
            const storedEquips = await db.getEquipments();

            if (storedEquips && storedEquips.length > 0) {
                const term = keyword.toString();
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

    // 默认回退保留既有行为：仅请求防具首屏 50 条。
    try {
        const data = await fetchRemoteEquipmentPage('armor', keyword, 1, 50, {});
        return data?.list || [];
    } catch {
        return [];
    }
}
