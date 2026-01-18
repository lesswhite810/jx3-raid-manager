# Read the file and verify 95-level raids
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'r', encoding='utf-8') as f:
    content = f.read()

import re

# Verify each section
sections = [
    ("剑胆琴心", r'// 剑胆琴心 \(95级初期\).*?// 风骨霸刀 \(95级中期\)'),
    ("风骨霸刀", r'// 风骨霸刀 \(95级中期\).*?// 重制版 \(95级后期\)'),
    ("重制版", r'// 重制版 \(95级后期\).*?// 世外蓬莱 \(100级\)')
]

for section_name, pattern in sections:
    print(f"\n=== {section_name} 副本验证 ===")
    section_match = re.search(pattern, content, re.DOTALL)
    if section_match:
        section_content = section_match.group()
        raid_entries = re.findall(r'\{\s*name:\s*[\'"]([^\'"]+)[\'"],\s*level:\s*\d+,\s*version:\s*[\'"][^\'"]+[\'"],\s*description:\s*[\'"]([^\'"]+)[\'"](?:,\s*isActive:\s*(false))?\s*\}', section_content)
        for i, (name, desc, is_active) in enumerate(raid_entries, 1):
            status = "默认禁用" if is_active else "默认启用"
            print(f"{i}. {name} - {desc} [{status}]")
        print(f"总计: {len(raid_entries)} 个副本")
    else:
        print("未找到该章节")
