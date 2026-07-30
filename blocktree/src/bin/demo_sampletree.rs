use blocktree::sampletree::{SampleTree, debug_print_tree};

fn main() {
    demo04();
}

fn demo01() {
    let mut prng = Prng::new();
    let mut tree = SampleTree::new(10);
    for _ in 0..2000 {
        // let i = (prng.next() as u16) as u64;
        let i = (prng.next() as u8) as u64;
        tree.insert(i, 100*i);
    }
    println!("------ final tree...");
    debug_print_tree(&tree, .., None);
}

fn demo02() {
    for i in 0..8 {
        sample(i);
    }
}

fn sample(iv: u32) {
    let mut prng = Prng::new();
    prng.a = iv;
    let mut tree = SampleTree::new(10);
    for _ in 0..50000 {
        let i = (prng.next() as u8) as u64 >> 3;
        let c = tree.get(i).unwrap_or(0) + 1;
        tree.insert(i, c);
    }
    // println!("------ final tree...");
    // debug_print_tree(&tree, .., None);
    println!("[iv {iv}] tree: len {}, height {}", tree.len, tree.height);
    println!("{:?}", tree.iter().into_iter().map(|p| p.1).collect::<Vec<_>>());
}

fn demo03() {
    use std::time::Instant;
    let i = 100000;
    println!("iterations: {i}");
    let start = Instant::now();
    demo03_tree(i);
    let t = Instant::now().duration_since(start);
    println!("tree: {}s", t.as_secs_f64());
    let start = Instant::now();
    demo03_array(i);
    let t = Instant::now().duration_since(start);
    println!("array: {}s", t.as_secs_f64());
}

fn demo03_tree(i: usize) {
    let mut prng = Prng::new();
    let mut tree = SampleTree::new(10);
    for _ in 0..i {
        // let i = (prng.next() as u16) as u64;
        let i = (prng.next() as u8) as u64;
        tree.update_or_insert(i, |v| *v += 1, || 1);
        // let c = tree.get(i).unwrap_or(0) + 1;
        // tree.insert(i, c);
    }
    println!("------ final tree...");
    // debug_print_tree(&tree, .., None);
    println!("{:?}", tree.iter().into_iter().map(|p| p.1).collect::<Vec<_>>());
}

fn demo03_array(i: usize) {
    let mut prng = Prng::new();
    let mut data = [0u64; 256];
    for _ in 0..i {
        // let i = (prng.next() as u16) as u64;
        let i = (prng.next() as u8) as u64;
        data[i as usize] += 1;
        // let c = tree.get(i).unwrap_or(0) + 1;
        // tree.insert(i, c);
    }
    println!("------ final tree...");
    // debug_print_tree(&tree, .., None);
    println!("{data:?}");
}

fn demo04() {
    let mut prng = Prng::new();
    let mut tree = SampleTree::new(10);
    let m = 1024;
    for _ in 0..100000 {
        let i = (prng.next() % m) as u64;
        tree.update_or_insert(i, |v| *v += 1, || 1);
    }
    let inspect_key = 197;
    for i in 0..m {
        // if i > inspect_key + 1 { panic!(); }
        // if i >= inspect_key {
        //     debug_print_tree(&tree, .., None);
        //     // panic!();
        // }
        if i % 4 != 0 {
            debug_print_tree(&tree, ..tree.height, Some(127));
            print!("### removing {i}...\n");
            match tree.remove(i) {
                Some(_) => (),
                None => {
                    debug_print_tree(&tree, .., None);
                    print!("unable to remove key {i}...\n");
                    panic!();
                }
            }
        }
    }
    debug_print_tree(&tree, .., None);
    println!("{:?}", tree.iter().into_iter().map(|p| p.0).collect::<Vec<_>>());
}

fn demo04_02() {
    let mut prng = Prng::new();
    let mut tree = SampleTree::new(10);
    let m = 1024;
    for _ in 0..100000 {
        let i = (prng.next() % m) as u64;
        tree.update_or_insert(i, |v| *v += 1, || 1);
    }
    for i in 0..m {
        // if i > 53 { panic!(); }
        if i % 4 != 0 {
            if i >= 187 {
                debug_print_tree(&tree, ..=3, Some(176));
                print!("### removing {i}...\n");
                // panic!();
            }
            if i > 189 { panic!(); }
            tree.remove(i);
        }
    }
    debug_print_tree(&tree, .., None);
    println!("{:?}", tree.iter().into_iter().map(|p| p.0).collect::<Vec<_>>());
}

fn demo05() {
    let mut prng = Prng::new();
    let mut tree = SampleTree::new(10);
    let m = 128;
    for _ in 0..1000 {
        let i = (prng.next() % m) as u64;
        tree.update_or_insert(i, |v| *v += 1, || 1);
    }
    for i in 0..m {
        if i % 4 != 0 {
            tree.remove(i);
        }
    }
    println!("iter;\t\t{:?}", tree.iter().into_iter().map(|p| p.0).collect::<Vec<_>>());
    let out = tree.iter_range(16..=22).into_iter().map(|p| p.0).collect::<Vec<_>>();
    println!("iter_range;\t{out:?}");
    // for i in 0..m {
    //     for j in i+1..i+3 {
    //         let out = tree.iter_range(i..j).into_iter().map(|p| p.0).collect::<Vec<_>>();
    //         println!("iter_range({i}..{j}) -> {out:?}");
    //     }
    // }
}

struct Prng {
    a: u32,
    b: u32,
    c: u32,
    d: u32,
}

impl Prng {
    fn new() -> Self {
        Self {
            a: 0,
            b: 0,
            c: 0,
            d: 0,
        }
    }
    fn next(&mut self) -> u64 {
        for i in 0..17 {
            self.c ^= self.a ^ 0xC9C9_C9C9;
            self.d ^= self.b ^ 0x3131_3131;
            (self.a, self.b) = self.c.carrying_mul(self.d, i * 1123);
            self.c ^= self.b ^ 0x7A7A_7A7A;
            self.d ^= self.a ^ 0xDEAD_BEEF;
            (self.a, self.b) = self.c.carrying_mul(self.d, i * 887);
        }
        (self.a as u64) << 32 | self.b as u64
    }
}