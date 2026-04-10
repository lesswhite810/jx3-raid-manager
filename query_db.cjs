const sqlite3 = require('sqlite3');

const dbPath = 'E:\\Game\\SeasunGame\\Game\\JX3\\bin\\zhcn_hd\\Interface\\MY#DATA\\!all-users@zhcn_hd\\userdata\\role_statistics\\equip_stat.v4.db';

const db = new sqlite3.Database(dbPath);

db.all("SELECT ownerkey, ownername, servername, ownerforce, ownersuitindex FROM OwnerInfo WHERE servername = '乾坤一掷'", (err, rows) => {
    if (err) {
        console.error(err);
    } else {
        console.log('=== Roles on 乾坤一掷 ===');
        console.log(JSON.stringify(rows, null, 2));
    }
    db.close();
});