// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.
//

// A test that needs to do cleanup if an array access is out of bounds.

#![allow(non_snake_case)]

#[macro_use]
extern crate mirai_annotations;

pub mod foreign_contracts {
    pub mod core {
        pub mod convert {
            pub mod From {
                pub fn from() -> String {
                    result!()
                }
            }
        }
    }
}

pub fn foo(arr: &mut [i32], i: usize) -> String {
    arr[i] = 123; //~ possible index out of bounds
    let result = String::from("foo"); // allocate something that needs explicit cleanup
    let _e = arr[i]; // no warning here because we can't get here unless line 27 succeeded
    result
}

pub fn main() {}
