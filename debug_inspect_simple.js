const https = require('https');

const url = 'https://node.jx3box.com/api/node/item/search?keyword=无修&page=1&per=1&client=std';

https.get(url, (res) => {
    let data = '';
    res.on('data', (chunk) => data += chunk);
    res.on('end', () => {
        try {
            const json = JSON.parse(data);
            const item = json.data.data[0];
            console.log(JSON.stringify(item.attributes, null, 2));
            if (item.attributes) {
                item.attributes.forEach(attr => {
                    console.log(`Type: ${attr.type}, Label: ${attr.label}, Color: ${attr.color}`);
                    // Check for atSkillEventHandler special formatting if any
                });
            }
        } catch (e) {
            console.error(e);
        }
    });
}).on('error', (e) => {
    console.error(e);
});
