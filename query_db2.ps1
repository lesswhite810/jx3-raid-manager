$OutputEncoding = [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$dbPath = "E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\Interface\MY#DATA\!all-users@zhcn_hd\userdata\role_statistics\equip_stat.v4.db"

$query = @"
SELECT ownerkey, ownername, servername, ownerforce, ownersuitindex FROM OwnerInfo WHERE servername = '乾坤一掷'
"@

$result = & sqlite3 $dbPath $query
$result | Out-File -FilePath "query_result.txt" -Encoding UTF8
Get-Content "query_result.txt"