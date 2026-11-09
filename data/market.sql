BEGIN TRANSACTION;
CREATE TABLE orders (
    order_id INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_phone TEXT NOT NULL,
    customer_name TEXT NOT NULL,
    sku TEXT NOT NULL,
    item_name TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    unit_price REAL NOT NULL,
    total_price REAL NOT NULL,
    address TEXT NOT NULL,
    order_date TEXT NOT NULL
);
CREATE TABLE products (
    sku TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    price REAL NOT NULL,
    currency TEXT NOT NULL DEFAULT 'EGP',
    material TEXT NOT NULL,
    sizes TEXT NOT NULL,
    colors TEXT NOT NULL,
    stock_quantity INTEGER NOT NULL,
    category TEXT NOT NULL,
    description TEXT
);
INSERT INTO "products" VALUES('cX23','تيشيرت أسود فاخر (Premium Black Tee)',400.0,'EGP','قطن مصري سوبر فاخر 100%','M, L, XL','أسود ملكي',18,'t-shirts','تيشيرت فاخر بخامة قطنية ثقيلة وملمس فائق النعومة');
INSERT INTO "products" VALUES('cx23','تيشيرت أسود اقتصادي (Economy Black Tee)',300.0,'EGP','مخلوط قطن بوليستر 65/35','S, M, L','أسود عادي',30,'t-shirts','تيشيرت يومي عملي بسعر اقتصادي مناسب للاستخدام المتكرر');
INSERT INTO "products" VALUES('pl10','قميص بولو كحلي كلاسيك (Classic Navy Polo)',450.0,'EGP','قطن بيكيه فاخر مع ياقة مقواة','L, XL, XXL','كحلي داكن',12,'polo','بولو كلاسيك راقي خفيف ومناسب للعمل والخروجات الصيفية');
INSERT INTO "products" VALUES('sh05','بنطلون جبردين زيتي سليم فت (Slim Fit Chino)',550.0,'EGP','جبردين تركي قطن ليكرا','32, 34, 36, 38','زيتي عسكري',8,'pants','بنطلون شينو أنيق مع نسبة ليكرا لراحة الحركة اليومية');
INSERT INTO "products" VALUES('jk99','جاكيت ووتربروف شتوي مبطن (Thermal Winter Jacket)',950.0,'EGP','ووتربروف عازل للمطر مع بطانة فايبر','M, L, XL','كحلي، رصاصي',6,'jackets','جاكيت شتوي دافئ مقاوم للرياح والأمطار مع جيوب حرارية');
DELETE FROM "sqlite_sequence";
COMMIT;
