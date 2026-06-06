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
    _IconID?: number;
    _Attrs?: string[];
    _AttrType?: string[];
    _Duty?: number;
    _PvType?: number;
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

/**
 * 获取指定装备数据
 */
export async function getEquip(keyword: string | number): Promise<JX3Equip[]> {
    const isId = !isNaN(Number(keyword));

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

    // Fallback: 直接从 API 获取
    try {
        const url = new URL('https://node.jx3box.com/equip/armor');
        url.searchParams.append('name', keyword.toString());
        url.searchParams.append('client', 'std');
        url.searchParams.append('page', '1');
        url.searchParams.append('per', '50');

        const response = await fetch(url.toString(), { method: 'GET' });

        if (!response.ok) {
            console.warn(`Failed to fetch items: ${response.status}`);
            return [];
        }

        const data = await response.json() as JX3EquipResponse;
        return data.list || [];

    } catch (error) {
        console.error(`Error fetching equipment:`, error);
        return [];
    }
}
