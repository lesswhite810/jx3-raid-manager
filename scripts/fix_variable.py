# Read the file
with open('e:/Data/jx3-raid-manager/components/RaidManager.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

# Find and replace the specific line
content = content.replace(
    '      const mergedRaids: MergedRaid[] = [];\n      const raidMap = new Map<string, Raid[]>();',
    '      const mergedRaids: MergedRaid[] = [];'
)

# Write the modified content
with open('e:/Data/jx3-raid-manager/components/RaidManager.tsx', 'w', encoding='utf-8') as f:
    f.write(content)

print("Removed unused variable!")
