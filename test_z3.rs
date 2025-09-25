use z3::{Context, Config, ast::{BV, Ast}};

fn main() {
    let cfg = Config::new();
    let ctx = Context::new(&cfg);
    
    // Test different ways to create BV
    let bv1 = BV::new_const(&ctx, "test", 256);
    println!("BV created: {}", bv1);
    
    // Test from_int
    let bv2 = BV::from_int(&ctx, 0, 256);
    println!("BV from int: {}", bv2);
    
    // Test simplify
    let simplified = bv1.simplify();
    println!("Simplified: {}", simplified);
}
