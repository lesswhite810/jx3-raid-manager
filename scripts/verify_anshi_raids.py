# Read the file and verify Anshi raids
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'r', encoding='utf-8') as f:
    content = f.read()

# Extract Anshi section
import re
start_idx = content.find('// 安史之乱 (90级)')
end_idx = content.find('// 剑胆琴心 (95级初期)')
anshi_section = content[start_idx:end_idx]

# Extract raid entries
raid_entries = re.findall(r'\{\s*name:\s*[\'"]([^\'"]+)[\'"],\s*level:\s*\d+,\s*version:\s*[\'"]([^\'"]+)[\'"],\s*description:\s*[\'"]([^\'"]+)[\'"](?:,\s*isActive:\s*(false))?\s*\}', anshi_section)

print("=== 安史之乱优化后的副本 ===")
for i, (name, version, desc, is_active) in enumerate(raid_entries, 1):
    status = "默认禁用" if is_active else "默认启用"
    print(f"{i}. {name} - {desc} [{status}]")

print(f"\n总计: {len(raid_entries)} 个副本")

# Verify against requirements
required_raids = [
    ("战宝军械库", "10人普通"),
    ("战宝军械库", "25人普通"),
    ("战宝军械库", "25人英雄"),
    ("大明宫", "10人普通"),
    ("大明宫", "25人普通"),
    ("大明宫", "25人英雄"),
    ("血战天策", "10人普通"),
    ("血战天策", "25人普通"),
    ("血战天策", "25人英雄"),
    ("风雪稻香村", "10人普通"),
    ("风雪稻香村", "25人普通"),
    ("风雪稻香村", "25人英雄"),
    ("秦皇陵", "10人普通"),
    ("秦皇陵", "25人普通"),
    ("秦皇陵", "25人英雄"),
    ("夜守孤城", "10人普通"),
    ("夜守孤城", "25人普通"),
    ("夜守孤城", "25人英雄"),
    ("夜守孤城", "25人挑战"),
    ("逐虎驱狼", "10人普通"),
    ("逐虎驱狼", "25人普通"),
    ("逐虎驱狼", "25人英雄"),
    ("逐虎驱狼", "25人挑战")
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
