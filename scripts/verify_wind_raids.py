# Read the file and check the wind section
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Find the wind section
in_wind_section = False
wind_raids = []

for i, line in enumerate(lines):
    if '// 风起稻香 (50-70级)' in line:
        in_wind_section = True
        print(f"Line {i+1}: {line.strip()}")
    elif in_wind_section and '// 巴蜀风云 (80级)' in line:
        in_wind_section = False
        break
    elif in_wind_section:
        if 'name:' in line and 'description:' in lines[i+2]:
            # Extract raid info
            name = line.split("name:")[1].split(",")[0].strip().strip("'\"")
            desc_line = lines[i+2]
            desc = desc_line.split("description:")[1].split(",")[0].strip().strip("'\"")
            wind_raids.append(f"{name}: {desc}")

print("\n=== 保留的风起稻香副本 ===")
for raid in wind_raids:
    print(f"✓ {raid}")

print(f"\n总计: {len(wind_raids)} 个副本")
