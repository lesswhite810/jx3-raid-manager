import re

# Read the file
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'r', encoding='utf-8') as f:
    lines = f.readlines()

result_lines = []
in_static_raids = False
current_entry = []
entry_start_idx = 0

for i, line in enumerate(lines):
    if 'STATIC_RAIDS' in line and '[' in line:
        in_static_raids = True
    
    if in_static_raids:
        current_entry.append(line)
        
        if '{' in line:
            entry_start_idx = i
        if '}' in line and ',' in line:
            # End of an entry
            entry_text = ''.join(current_entry)
            
            # Check if this is a 25人 entry (but not 10人)
            if '25人' in entry_text and '10人' not in entry_text:
                # Check if it's NOT 弓月城 or 认罪之渊
                if '弓月城' not in entry_text and '认罪之渊' not in entry_text:
                    # Add isActive: false before the closing brace
                    # Find the line with closing brace
                    entry_lines = current_entry[:-1]  # All lines except closing brace
                    closing_brace_line = current_entry[-1]
                    
                    # Check if isActive already exists
                    if 'isActive' not in entry_text:
                        # Find the last line before closing brace
                        last_content_line = None
                        for j in range(len(entry_lines) - 1, -1, -1):
                            if entry_lines[j].strip() and not entry_lines[j].strip().startswith('//'):
                                last_content_line = entry_lines[j]
                                break
                        
                        if last_content_line:
                            # Add isActive: false after the last content line
                            indent = '    '
                            if last_content_line.strip().endswith(','):
                                result_lines.extend(entry_lines[:-1])
                                result_lines.append(last_content_line.rstrip() + '\n')
                                result_lines.append(f'{indent}isActive: false,\n')
                            else:
                                result_lines.extend(entry_lines[:-1])
                                result_lines.append(last_content_line.rstrip() + ',\n')
                                result_lines.append(f'{indent}isActive: false,\n')
                            result_lines.append(closing_brace_line)
                            current_entry = []
                            continue
            
            # Just add the entry as is
            result_lines.extend(current_entry)
            current_entry = []
    else:
        result_lines.append(line)

# Write the modified content
with open('e:/Data/jx3-raid-manager/data/staticRaids.ts', 'w', encoding='utf-8') as f:
    f.writelines(result_lines)

print("Done!")
