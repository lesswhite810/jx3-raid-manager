# Read the file
with open('e:/Data/jx3-raid-manager/components/RaidManager.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

# Find the line with the old content and replace it
old_content = '''                      <div className="flex items-center gap-2 mb-3 flex-wrap justify-center sm:justify-start">
                        {mergedRaid.raids.map(raid => {
                          const label = mergedRaid.difficultyLabels[getRaidKey(raid)] || DIFFICULTY_LABELS[raid.difficulty];
                          return (
                            <div 
                              key={getRaidKey(raid)}
                              onClick={() => onRaidClick?.(raid)}
                              onContextMenu={(e) => {
                                e.preventDefault();
                                toggleRaidDifficultyStatus(getRaidKey(raid));
                              }}
                              className={`relative px-3 py-2 text-xs sm:text-sm font-bold rounded-lg border cursor-pointer transition-all hover:scale-105 hover:shadow-sm min-w-[60px] sm:min-w-[70px] text-center ${
                                raid.isActive 
                                  ? DIFFICULTY_COLORS[raid.difficulty] 
                                  : 'bg-slate-50 text-slate-400 border-slate-200 hover:bg-slate-100 opacity-60'
                              }`}
                              title={raid.isActive ? `${label} - 点击进入详情，右键切换状态` : `已禁用 - ${label} - 右键启用`}
                            >
                              {label}
                              {!raid.isActive && (
                                <div className="absolute -top-1 -right-1 w-2.5 h-2.5 bg-red-400 rounded-full border border-white" />
                              )}
                            </div>
                          );
                        })}
                      </div>'''

new_content = '''                      <div className="space-y-3">
                        {/* 10人副本 */}
                        {mergedRaid.raids.filter(raid => raid.playerCount === 10).length > 0 && (
                          <div>
                            <div className="flex items-center gap-2 mb-2">
                              <div className="w-2 h-2 bg-blue-500 rounded-full"></div>
                              <span className="text-xs font-medium text-blue-600">10人副本</span>
                            </div>
                            <div className="flex items-center gap-2 mb-3 flex-wrap justify-center sm:justify-start">
                              {mergedRaid.raids.filter(raid => raid.playerCount === 10).map(raid => {
                                const label = mergedRaid.difficultyLabels[getRaidKey(raid)] || DIFFICULTY_LABELS[raid.difficulty];
                                return (
                                  <div 
                                    key={getRaidKey(raid)}
                                    onClick={() => onRaidClick?.(raid)}
                                    onContextMenu={(e) => {
                                      e.preventDefault();
                                      toggleRaidDifficultyStatus(getRaidKey(raid));
                                    }}
                                    className={`relative px-3 py-2 text-xs sm:text-sm font-bold rounded-lg border cursor-pointer transition-all hover:scale-105 hover:shadow-sm min-w-[60px] sm:min-w-[70px] text-center ${
                                      raid.isActive 
                                        ? DIFFICULTY_COLORS[raid.difficulty] 
                                        : 'bg-slate-50 text-slate-400 border-slate-200 hover:bg-slate-100 opacity-60'
                                    }`}
                                    title={raid.isActive ? `${label} - 点击进入详情，右键切换状态` : `已禁用 - ${label} - 右键启用`}
                                  >
                                    {label}
                                    {!raid.isActive && (
                                      <div className="absolute -top-1 -right-1 w-2.5 h-2.5 bg-red-400 rounded-full border border-white" />
                                    )}
                                  </div>
                                );
                              })}
                            </div>
                          </div>
                        )}
                        
                        {/* 25人副本 */}
                        {mergedRaid.raids.filter(raid => raid.playerCount === 25).length > 0 && (
                          <div>
                            <div className="flex items-center gap-2 mb-2">
                              <div className="w-2 h-2 bg-red-500 rounded-full"></div>
                              <span className="text-xs font-medium text-red-600">25人副本</span>
                              {mergedRaid.raids.filter(raid => raid.playerCount === 25).every(raid => !raid.isActive) && (
                                <span className="text-xs text-slate-500 ml-1">（默认禁用）</span>
                              )}
                            </div>
                            <div className="flex items-center gap-2 mb-3 flex-wrap justify-center sm:justify-start">
                              {mergedRaid.raids.filter(raid => raid.playerCount === 25).map(raid => {
                                const label = mergedRaid.difficultyLabels[getRaidKey(raid)] || DIFFICULTY_LABELS[raid.difficulty];
                                const isSpecialRaid = raid.name === '弓月城' || raid.name === '缚罪之渊';
                                return (
                                  <div 
                                    key={getRaidKey(raid)}
                                    onClick={() => onRaidClick?.(raid)}
                                    onContextMenu={(e) => {
                                      e.preventDefault();
                                      toggleRaidDifficultyStatus(getRaidKey(raid));
                                    }}
                                    className={`relative px-3 py-2 text-xs sm:text-sm font-bold rounded-lg border cursor-pointer transition-all hover:scale-105 hover:shadow-sm min-w-[60px] sm:min-w-[70px] text-center ${
                                      raid.isActive 
                                        ? DIFFICULTY_COLORS[raid.difficulty] 
                                        : 'bg-slate-50 text-slate-400 border-slate-200 hover:bg-slate-100 opacity-60'
                                    }`}
                                    title={raid.isActive ? `${label} - 点击进入详情，右键切换状态` : `已禁用 - ${label} - 右键启用${!raid.isActive && !isSpecialRaid ? '（默认禁用）' : ''}`}
                                  >
                                    {label}
                                    {!raid.isActive && (
                                      <div className="absolute -top-1 -right-1 w-2.5 h-2.5 bg-red-400 rounded-full border border-white" />
                                    )}
                                    {!raid.isActive && !isSpecialRaid && (
                                      <div className="absolute -top-1 -left-1 w-2.5 h-2.5 bg-orange-400 rounded-full border border-white" title="默认禁用" />
                                    )}
                                  </div>
                                );
                              })}
                            </div>
                          </div>
                        )}
                      </div>'''

# Replace the content
content = content.replace(old_content, new_content)

# Write the modified content
with open('e:/Data/jx3-raid-manager/components/RaidManager.tsx', 'w', encoding='utf-8') as f:
    f.write(content)

print("UI display updated successfully!")
