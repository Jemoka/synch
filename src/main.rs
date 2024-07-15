mod sync;
use sync::*;

use ciborium::{into_writer, from_reader};

fn main() {
    let mut list:SyncedList<f32> = SyncedList::new();
    list.push(3.02);

    let mut w:Vec<u8> = vec![];
    into_writer(&list.replay(), &mut w).unwrap();

    dbg!(w);

    // let mut list_a:List<String, String> = List::new();
    // let mut list_b:List<String, String> = list_a.clone();

    // // alice and bob puts something in their lists
    // let a_op_1 = list_a.insert_index(3, "H0".into(), "A".into());
    // list_a.apply(a_op_1.clone());
    // let b_op_1 = list_b.insert_index(3, "H2".into(), "B".into());
    // list_b.apply(b_op_1.clone());

    // // they sync
    // let pool = [a_op_1, b_op_1];
    // pool.into_iter().for_each(|x| {
    //     list_a.apply(x.clone());
    //     list_b.apply(x.clone());
    // });

    // dbg!(list_a.read::<Vec<_>>());
    // dbg!(list_b.read::<Vec<_>>());

    // // bob faints, alice delet's bob's thing and writes away
    // let a_op_2 = list_a.delete_index(1, "A".into()).unwrap();
    // list_a.apply(a_op_2.clone());
    // let a_op_3 = list_a.insert_index(1, "H4".into(), "A".into());
    // list_a.apply(a_op_3.clone());
    // let a_op_4 = list_a.insert_index(2, "H8".into(), "A".into());
    // list_a.apply(a_op_4.clone());

    // // bob wakes up, delet's alice's thing and insrts some stuff
    // let b_op_2 = list_b.delete_index(0, "B".into()).unwrap();
    // list_b.apply(b_op_2.clone());
    // let b_op_3 = list_b.insert_index(1, "H9".into(), "B".into());
    // list_b.apply(b_op_3.clone());

    // // they sync
    // let pool = [b_op_2, a_op_2, a_op_3, a_op_4, b_op_3];
    // pool.into_iter().for_each(|x| {
    //     list_a.apply(x.clone());
    //     list_b.apply(x.clone());
    // });

    // dbg!(list_a.read::<Vec<_>>());
    // dbg!(list_b.read::<Vec<_>>());


    // let test_list:List<String, String> = from_reader(w.as_slice()).unwrap();
    // dbg!(test_list.read::<Vec<_>>());
}
