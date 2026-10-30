use prusti_contracts::*;

//#![feature(nll)]

struct List {
    next: Option<Box<List>>,
}

fn use_list(_l: &mut List) {}

fn test(l: &mut List) -> &mut List {
    let mut c = l;
    let mut end = false;
    while !end {
        match c.next {
            Some(ref mut next) => {
                c = next;
            },
            None => {
                end = true;
            }
        }
    }
    use_list(c);
    c
}

/*
fn test2(mut l: List) {
    let mut c = &mut l;
    let mut end = false;
    while !end {
        match c.next {
            Some(ref mut next) => {
                c = next;
            },
            None => {
                end = true;
            }
        }
    }
    use_list(c);
}

fn test3(l: &mut List) -> &mut List {
    if let Some(ref mut c) = l.next {
        let mut c = c;
        let mut end = false;
        while !end {
            match c.next {
                Some(ref mut next) => {
                    c = next;
                },
                None => {
                    end = true;
                }
            }
        }
        use_list(c);
        c
    } else {
        l
    }
}
*/

fn main() {
    //test(List {next: None});
    test(&mut List {next: None});
}

struct RefList<'a> {
    value: u32,
    next: Option<&'a mut RefList<'a>>,
}

fn destructive_reborrowing<'a, 'b>(l: &'b mut RefList<'a>) -> &'b mut RefList<'a> {
    let mut curr = l;
    loop {
        match curr.next {
            Some(ref mut next) => {
                curr = next;
            }
            None => {
                break;
            }
        }
    }
    curr
}

fn use_ref(_r: &mut i32) {}

fn lifetime_gaps() {
    let mut y = 5;
    let mut x;

    x = &mut y;
    use_ref(x);

    x = &mut y;
    use_ref(x);
}

fn reference_parameters<'a>(x: &'a mut u32, y: &'a mut u32) -> &'a mut u32 {
	x
}

struct T {}

fn use_t(_x: &mut T) {}

fn split_borrows(mut a: T, mut b: T, mut d: T, mut c: bool) {
    let mut x = &mut a;
    let mut y = &mut d;
    while c {
        if !c {
            x = y;
            y = &mut a;
            x = &mut b;
        }
        c = false;
    }
    use_t(x);
    use_t(y);
}

/*
fn test(mut a: T, mut b: T, mut d: T, mut c: bool) {
    let mut x = &mut a;
    let mut y = &mut d;
    while c {
        if !c {
            x = &mut b;
            y = &mut a;
        }
        c = false;
    }
    use_t(x);
    use_t(y);
}

fn test(mut a: T, mut b: T, mut c: bool) {
    let mut x = &mut a;
    while c {
        if !c {
            x = &mut b;
        }
        c = false;
    }
    use_t(x);
}
*/
