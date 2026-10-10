BEGIN;
CREATE TABLE customers (
 id integer PRIMARY KEY, company_name text NOT NULL, email text, country text NOT NULL,
 notes text, created_at timestamp NOT NULL DEFAULT '2024-01-01'
);
CREATE TABLE orders (
 id integer PRIMARY KEY, customer_id integer NOT NULL REFERENCES customers(id),
 order_date date NOT NULL, freight numeric(12,2) NOT NULL, status text NOT NULL
);
CREATE TABLE products (
 id integer PRIMARY KEY, name text NOT NULL, unit_price numeric(12,2) NOT NULL
);
CREATE TABLE line_items (
 id integer PRIMARY KEY, order_id integer NOT NULL REFERENCES orders(id),
 product_id integer NOT NULL REFERENCES products(id), quantity integer NOT NULL,
 unit_price numeric(12,2) NOT NULL, discount numeric(5,2) NOT NULL DEFAULT 0
);
CREATE TABLE contacts (
 id integer PRIMARY KEY, customer_id integer NOT NULL REFERENCES customers(id),
 name text NOT NULL, phone text
);
INSERT INTO customers(id,company_name,email,country,notes) VALUES
 (1,'Familia Arquibaldo','familia@example.test','Brazil',NULL),
 (2,'Folies gourmandes','folies@example.test','France',NULL),
 (3,'Nordlys Handel','nordlys@example.test','Norway',NULL),
 (4,'No Orders Yet',NULL,'Norway',NULL),
 (5,'東京商店','tokyo@example.test','Japan',NULL);
INSERT INTO customers(id,company_name,email,country)
 SELECT n, 'Customer ' || lpad(n::text,3,'0'), 'customer' || n || '@example.test',
 CASE WHEN n % 2=0 THEN 'Norway' ELSE 'France' END FROM generate_series(6,125) n;
INSERT INTO products VALUES
 (1,'Jack''s New England Clam Chowder',9.65),(2,'Filo Mix',7.00),
 (3,'Pâté chinois',24.00),(4,'Rhönbräu Klosterbier',7.75),
 (5,'Geitost',2.00),(6,'Guaraná Fantástica',3.60);
INSERT INTO orders VALUES
 (101,1,'2024-12-01',10.83,'Shipped'),(102,1,'2024-09-29',176.81,'Pending'),
 (103,1,'2024-07-27',3.01,'Delivered'),(104,2,'2025-01-22',100.60,'Shipped'),
 (105,3,'2025-01-03',0.00,'Pending'),(106,5,'2025-02-01',12.00,'Shipped');
INSERT INTO orders SELECT 1000+n,n,'2025-01-01'::date + n, n/10.0,'Delivered'
 FROM generate_series(6,125) n;
INSERT INTO line_items VALUES
 (1,101,1,12,9.65,0),(2,101,2,4,7.00,0),(3,101,3,6,24.00,0),
 (4,102,4,30,7.75,0.05),(5,102,5,50,2.00,0),
 (6,103,4,50,7.75,0.20),(7,104,6,20,3.60,0.15),
 (8,104,3,10,24.00,0),(9,106,5,9,2.00,0);
INSERT INTO line_items SELECT 1000+n,1000+n,1,2,9.65,0 FROM generate_series(6,125) n;
INSERT INTO contacts VALUES
 (1,1,'Ana','+55 111'),(2,1,'Bruno',NULL),(3,2,'Claire','+33 222'),(4,3,'Åse','+47 333');
CREATE INDEX ON orders(customer_id);
CREATE INDEX ON line_items(order_id);
CREATE INDEX ON contacts(customer_id);
COMMIT;
