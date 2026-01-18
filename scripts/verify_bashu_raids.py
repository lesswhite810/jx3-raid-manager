# Read the file and verify Bashu raids
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'r', encoding='utf-8') as f:
    content = f.read()

# Extract Bashu section
import re
start_idx = content.find('// 巴蜀风云 (80级)')
end_idx = content.find('// 安史之乱 (90级)')
bashu_section = content[start_idx:end_idx]

# Extract raid entries
raid_entries = re.findall(r'\{\s*name:\s*[\'"]([^\'"]+)[\'"],\s*level:\s*\d+,\s*version:\s*[\'"]([^\'"]+)[\'"],\s*description:\s*[\'"]([^\'"]+)[\'"](?:,\s*isActive:\s*(false))?\s*\}', bashu_section)

print("=== 巴蜀风云优化后的副本 ===")
for i, (name, version, desc, is_active) in enumerate(raid_entries, 1):
    status = "默认禁用" if is_active else "默认启用"
    print(f"{i}. {name} - {desc} [{status}]")

print(f"\n总计: {len(raid_entries)} 个副本")

# Verify against requirements
required_raids = [
    ("龙渊泽", "10人普通"),
    ("龙渊泽", "10人英雄"),
    ("龙渊泽", "25人普通"),
    ("龙渊泽", "25人英雄"),
    ("荻花圣殿", "10人普通"),
    ("荻花圣殿", "10人英雄"),
    ("荻花圣殿", "25人普通"),
    ("荻花圣殿", "25人英雄"),
    ("烛龙殿", "10人普通"),
    ("烛龙殿", "25人普通"),
    ("烛龙殿", "25人英雄"),
    ("持国回忆录", "10人普通"),
    ("持国回忆录", "25人英雄"),
    ("会战唐门", "10人普通"),
    ("会战唐门", "25人英雄"),
    ("南诏皇宫", "10人普通"),
    ("南诏皇宫", "25人普通"),
    ("南诏皇宫", "25人英雄")
]

print("\n=== 验证结果 ===")
all_found = True
for name, mode in required_raids:
    found = any(name == raid_name and mode in desc for raid_name, _, desc, _ in raid_entries)
    status = "✓" if found else "✗"
    print(f"{status} {name} - {mode}")
    if not found:
        all_found = False

print(f"\n总体验证: {'✓ 全部符合要求' if all_found else '✗ 有缺失'}")
