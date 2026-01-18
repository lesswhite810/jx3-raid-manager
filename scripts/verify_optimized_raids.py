# Read the file and check the wind section
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'r', encoding='utf-8') as f:
    content = f.read()

# Find the wind section
start_idx = content.find('// 风起稻香 (50-70级)')
end_idx = content.find('// 巴蜀风云 (80级)')

wind_section = content[start_idx:end_idx]

# Extract raid entries
import re
raid_entries = re.findall(r'\{\s*name:\s*[\'"]([^\'"]+)[\'"],\s*level:\s*\d+,\s*version:\s*[\'"]风起稻香[\'"],\s*description:\s*[\'"]([^\'"]+)[\'"](?:,\s*isActive:\s*(false))?\s*\}', wind_section)

print("=== 风起稻香保留的副本 ===")
for i, (name, desc, is_active) in enumerate(raid_entries, 1):
    status = "默认禁用" if is_active else "默认启用"
    print(f"{i}. {name} - {desc} [{status}]")

print(f"\n总计: {len(raid_entries)} 个副本")

# Verify against requirements
required_raids = [
    ("战宝迦兰", "10人普通"),
    ("战宝迦兰", "25人英雄"), 
    ("荻花宫后山", "10人普通"),
    ("宫中神武遗迹", "10人普通"),
    ("宫中神武遗迹", "25人英雄"),
    ("持国天王殿", "10人普通")
]

print("\n=== 验证结果 ===")
all_found = True
for name, mode in required_raids:
    found = any(name == raid_name and mode in desc for raid_name, desc, _ in raid_entries)
    status = "✓" if found else "✗"
    print(f"{status} {name} - {mode}")
    if not found:
        all_found = False

print(f"\n总体验证: {'✓ 全部符合要求' if all_found else '✗ 有缺失'}")
