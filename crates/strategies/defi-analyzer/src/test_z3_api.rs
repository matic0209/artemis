//! Test Z3 API usage

use z3::{Context, Config, ast::{BV, Ast}};

pub fn test_z3_api() {
    let cfg = Config::new();
    let ctx = Context::new(&cfg);
    
    // Test BV creation methods
    let bv1 = BV::new_const(&ctx, "test", 256);
    let bv2 = BV::from_int(&ctx, 0, 256);
    
    // Test operations
    let bv3 = bv1._eq(&bv2);
    let simplified = bv1.simplify();
    
    println!("BV1: {}", bv1);
    println!("BV2: {}", bv2);
    println!("BV3: {}", bv3);
    println!("Simplified: {}", simplified);
}
