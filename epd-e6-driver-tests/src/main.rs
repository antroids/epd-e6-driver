fn main() {
    println!("Run with cargo test -p epd-e6-driver-tests --target x86_64-unknown-linux-gnu");
}

#[cfg(test)]
mod tests {
    use epd_e6_driver::prelude::*;

    #[test]
    #[should_panic]
    fn vec_nibbles_not_enough_space_test() {
        let _: Nibbles<_, u8> = Nibbles::new(vec![0u8; 32], 65);
    }

    #[test]
    fn vec_nibbles_enough_space_test() {
        let nibbles: Nibbles<_, u8> = Nibbles::new(vec![0u8; 32], 64);
        assert_eq!(nibbles.get(63), 0);
    }

    #[test]
    #[should_panic]
    fn vec_nibbles_index_out_of_bounds() {
        let nibbles: Nibbles<_, u8> = Nibbles::new(vec![0u8; 32], 64);
        nibbles.get(64);
    }

    #[test]
    #[should_panic]
    fn array_nibbles_not_enough_space_test() {
        let _: Nibbles<_, u8> = Nibbles::new([0u8; 32], 65);
    }

    #[test]
    fn get_set_nibbles_test() {
        let mut nibbles: Nibbles<_, u8> = Nibbles::new([0u8; 4], 7);
        nibbles.into_iter().for_each(|nibble| assert_eq!(nibble, 0));

        for i in 0..nibbles.len() {
            nibbles.set(i, i as u8 % 0x0F);
        }

        for i in 0..nibbles.len() {
            assert_eq!(nibbles.get(i), i as u8 % 0x0F);
        }
    }
}
