$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Data

$dbPath = "E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\Interface\MY#DATA\!all-users@zhcn_hd\userdata\role_statistics\equip_stat.v4.db"

$conn = New-Object System.Data.SQLite.SQLiteConnection
$conn.ConnectionString = "Data Source=$dbPath;Version=3;"
$conn.Open()

$cmd = $conn.CreateCommand()
$cmd.CommandText = "SELECT ownerkey, ownername, servername, ownerforce, ownersuitindex FROM OwnerInfo WHERE servername = '乾坤一掷'"

$reader = $cmd.ExecuteReader()
while ($reader.Read()) {
    $ownerkey = $reader.GetString(0)
    $ownername = $reader.GetString(1)
    $servername = $reader.GetString(2)
    $ownerforce = $reader.GetInt32(3)
    $ownersuitindex = if ($reader.IsDBNull(4)) { "NULL" } else { $reader.GetInt32(4) }
    Write-Host "ownerkey=$ownerkey, ownername=$ownername, servername=$servername, ownerforce=$ownerforce, ownersuitindex=$ownersuitindex"
}

$reader.Close()
$conn.Close()