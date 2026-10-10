use crate::{schema::*, Result};
use postgres::Client;

pub fn discover(client: &mut Client) -> Result<Catalog> {
    let mut catalog = Catalog::default();
    for row in client.query("SELECT n.nspname,c.relname FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE c.relkind IN ('r','p','v','m') AND n.nspname NOT IN ('pg_catalog','information_schema') AND n.nspname NOT LIKE 'pg_toast%' ORDER BY 1,2", &[])? {
        let name=TableName {schema:row.get(0),name:row.get(1)};
        let columns=client.query("SELECT a.attname,format_type(a.atttypid,a.atttypmod),NOT a.attnotnull FROM pg_attribute a WHERE a.attrelid=to_regclass($1) AND a.attnum>0 AND NOT a.attisdropped ORDER BY a.attnum", &[&name.sql()])?.into_iter().map(|r|Column{name:r.get(0),data_type:r.get(1),nullable:r.get(2)}).collect();
        let primary_keys=client.query("SELECT a.attname FROM pg_index i CROSS JOIN LATERAL unnest(i.indkey) WITH ORDINALITY k(attnum,ord) JOIN pg_attribute a ON a.attrelid=i.indrelid AND a.attnum=k.attnum WHERE i.indrelid=to_regclass($1) AND i.indisprimary ORDER BY k.ord", &[&name.sql()])?.into_iter().map(|r|r.get(0)).collect();
        catalog.tables.push(TableInfo{name,columns,primary_keys});
    }
    for row in client.query("SELECT co.conname, ns.nspname,cl.relname,nt.nspname,ct.relname, ARRAY(SELECT a.attname::text FROM unnest(co.conkey) WITH ORDINALITY k(num,ord) JOIN pg_attribute a ON a.attrelid=co.conrelid AND a.attnum=k.num ORDER BY ord), ARRAY(SELECT a.attname::text FROM unnest(co.confkey) WITH ORDINALITY k(num,ord) JOIN pg_attribute a ON a.attrelid=co.confrelid AND a.attnum=k.num ORDER BY ord) FROM pg_constraint co JOIN pg_class cl ON cl.oid=co.conrelid JOIN pg_namespace ns ON ns.oid=cl.relnamespace JOIN pg_class ct ON ct.oid=co.confrelid JOIN pg_namespace nt ON nt.oid=ct.relnamespace WHERE co.contype='f' ORDER BY ns.nspname,cl.relname,co.conname", &[])? {
        catalog.foreign_keys.push(ForeignKey{name:row.get(0),from:TableName{schema:row.get(1),name:row.get(2)},to:TableName{schema:row.get(3),name:row.get(4)},from_columns:row.get(5),to_columns:row.get(6)});
    }
    Ok(catalog)
}
