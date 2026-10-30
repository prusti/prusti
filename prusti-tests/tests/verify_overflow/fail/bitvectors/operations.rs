//@ compile-flags: -Pencode_bitvectors=true

use prusti_contracts::*;

fn bitadd_1() {
    let a = 1u8;
    let b = a | 2;
    let c = b + 4;
    assert!(c == 7);
}

fn bitadd_2() {
    let a = 1u8;
    let b = a | 2;
    let c = b + 4;
    assert!(c == 8);    //~ ERROR: might not hold
}

fn bitmul_1() {
    let a = 1u8;
    let b = a | 2;
    let c = b * 3;
    assert!(c == 9);
}

fn bitmul_2() {
    let a = 1u8;
    let b = a | 2;
    let c = b * 3;
    assert!(c == 8);    //~ ERROR: might not hold
}

fn bitdiv_1() {
    let a = 1u8;
    let b = a | 2;
    let c = b / 3;
    assert!(c == 1);
}

fn bitdiv_2() {
    let a = 1u8;
    let b = a | 2;
    let c = b / 3;
    assert!(c == 2);    //~ ERROR: might not hold
}

fn bitand_1() {
    let a = 3u8;
    let b = a & 2;
    assert!(b == 2);
}

fn bitand_2() {
    let a = 3u8;
    let b = a & 2;
    assert!(b == 3);    //~ ERROR: might not hold
}

fn bitor_1() {
    let a = 1u8;
    let b = a | 2;
    assert!(b == 3);
}

fn bitor_2() {
    let a = 1u8;
    let b = a | 2;
    assert!(b == 1);    //~ ERROR: might not hold
}

fn bitxor_1() {
    let a = 254u8;
    let b = a ^ 2;
    assert!(b == 252);
}

fn bitxor_2() {
    let a = 254u8;
    let b = a ^ 2;
    assert!(b == 253);  //~ ERROR: might not hold
}
