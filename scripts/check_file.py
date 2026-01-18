# Read and check the file
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'r', encoding='utf-8') as f:
    lines = f.readlines()

print(f"Total lines: {len(lines)}")

# Check last 30 lines
print("\n=== Last 30 lines ===")
for i in range(max(0, len(lines)-30), len(lines)):
    print(f"{i+1}: {lines[i].rstrip()}")

# Check for 认罪之渊
print("\n=== Lines containing 认罪之渊 ===")
for i, line in enumerate(lines):
    if '认罪之渊' in line:
        print(f"{i+1}: {line.rstrip()}")
