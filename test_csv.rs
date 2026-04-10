fn main() {
    let mut field_buf = String::from("test");
    let mut row = Vec::new();

    // Instead of row.push(field_buf.clone()); field_buf.clear();
    // We can do:
    row.push(std::mem::take(&mut field_buf));

    println!("{:?}", row);
    println!("{:?}", field_buf);
}
