-- Portable fixture for SQLite and DuckDB. Run only in a new, disposable database.
CREATE TABLE customers (id INTEGER PRIMARY KEY, name VARCHAR NOT NULL, notes VARCHAR);
CREATE TABLE orders (customer_id INTEGER NOT NULL REFERENCES customers(id), id INTEGER NOT NULL, status VARCHAR, PRIMARY KEY(customer_id,id));
CREATE TABLE items (id INTEGER PRIMARY KEY, customer_id INTEGER, order_id INTEGER, quantity INTEGER, price DECIMAL(12,3), FOREIGN KEY(customer_id,order_id) REFERENCES orders(customer_id,id));
CREATE TABLE contacts (id INTEGER PRIMARY KEY, customer_id INTEGER REFERENCES customers(id), name VARCHAR);
CREATE VIEW customer_names AS SELECT name FROM customers;
CREATE TABLE "odd ""table" (id INTEGER PRIMARY KEY, "odd column" VARCHAR);
INSERT INTO customers VALUES (1,'東京商店',NULL),(2,'Familia Arquibaldo',''),(3,'No orders','empty');
INSERT INTO orders VALUES (1,1,'Shipped'),(1,2,'Pending'),(2,1,'Pending');
INSERT INTO items VALUES (1,1,1,2,10.125),(2,1,1,3,1.125),(3,1,2,2,7.000),(4,2,1,1,5.000);
INSERT INTO contacts VALUES (1,1,'Alice'),(2,1,'Bob'),(3,2,'Carol');
INSERT INTO "odd ""table" VALUES (1,'hello');
