// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.
//

use crate::expression::ExpressionType::{self, *};

use log_derive::*;
use serde::{Deserialize, Serialize};
use std::cmp;

/// An element of the Interval domain is a range of i128 numbers denoted by a lower bound and
/// upper bound. A lower bound of i128::MIN denotes -infinity and an upper bound of
/// i128::MAX denotes +infinity.
/// Interval domain elements are constructed on demand from AbstractDomain expressions.
/// They are most useful for checking if an array index is within bounds.
#[derive(Serialize, Deserialize, Clone, Eq, PartialOrd, PartialEq, Hash, Ord)]
pub struct IntervalDomain {
    lower_bound: i128,
    upper_bound: i128,
}

impl std::fmt::Debug for IntervalDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.lower_bound, self.upper_bound) {
            (1, 0) => f.write_str("[bottom]"),
            (i128::MIN, i128::MAX) => f.write_str("[..]"),
            (i128::MIN, _) => f.write_fmt(format_args!("[..{}]", self.upper_bound)),
            (_, i128::MAX) => f.write_fmt(format_args!("[{}..]", self.lower_bound)),
            _ => f.write_fmt(format_args!("[{}..{}]", self.lower_bound, self.upper_bound)),
        }
    }
}

pub const BOTTOM: IntervalDomain = IntervalDomain {
    lower_bound: 1,
    upper_bound: 0,
};

pub const TOP: IntervalDomain = IntervalDomain {
    lower_bound: i128::MIN,
    upper_bound: i128::MAX,
};

impl From<i128> for IntervalDomain {
    #[logfn_inputs(TRACE)]
    fn from(i: i128) -> IntervalDomain {
        IntervalDomain {
            lower_bound: i,
            upper_bound: i,
        }
    }
}

impl From<u128> for IntervalDomain {
    #[logfn_inputs(TRACE)]
    fn from(u: u128) -> IntervalDomain {
        if let Result::Ok(i) = i128::try_from(u) {
            i.into()
        } else {
            IntervalDomain {
                lower_bound: i128::MAX,
                upper_bound: i128::MAX,
            }
        }
    }
}

impl From<ExpressionType> for IntervalDomain {
    #[logfn_inputs(TRACE)]
    fn from(t: ExpressionType) -> IntervalDomain {
        let (lower_bound, upper_bound) = match t {
            I8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
            I16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
            I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
            I64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
            I128 => (i128::MIN, i128::MAX),
            Isize => ((isize::MIN as i128), (isize::MAX as i128)),
            U8 => (0, i128::from(u8::MAX)),
            U16 => (0, i128::from(u16::MAX)),
            U32 => (0, i128::from(u32::MAX)),
            U64 => (0, i128::from(u64::MAX)),
            U128 => (0, i128::MAX),
            Usize => (0, (usize::MAX as i128)),
            _ => return BOTTOM.clone(),
        };
        IntervalDomain {
            lower_bound,
            upper_bound,
        }
    }
}

impl IntervalDomain {
    //[x...y] + [a...b] = [x+a...y+b]
    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn add(&self, other: &Self) -> Self {
        if self.is_bottom() || other.is_bottom() {
            return BOTTOM.clone();
        }
        if self.is_top() || other.is_top() {
            return TOP.clone();
        }
        IntervalDomain {
            lower_bound: self.lower_bound.saturating_add(other.lower_bound),
            upper_bound: self.upper_bound.saturating_add(other.upper_bound),
        }
    }

    //[x...y] / [a...b] = [x/b...y/a] if a > 0
    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn div(&self, other: &Self) -> Self {
        if self.is_bottom() || other.is_bottom() {
            return BOTTOM.clone();
        }
        if self.is_top() || other.is_top() {
            return TOP.clone();
        }
        if other.lower_bound > 0 {
            IntervalDomain {
                lower_bound: self.lower_bound / other.upper_bound,
                upper_bound: self.upper_bound / other.lower_bound,
            }
        } else {
            TOP.clone()
        }
    }

    // [x...y] >= [a...b] = x >= b
    // !([x...y] >= [a...b]) = [a...b] > [x...y] = a > y
    #[logfn_inputs(TRACE)]
    pub fn greater_or_equal(&self, other: &Self) -> Option<bool> {
        if self.is_bottom() || self.is_top() || other.is_bottom() || other.is_top() {
            None
        } else if self.lower_bound >= other.upper_bound {
            Some(true)
        } else if other.lower_bound > self.upper_bound {
            Some(false)
        } else {
            None
        }
    }

    // [x...y] > [a...b] = x > b
    // !([x...y] > [a...b]) = [a...b] >= [x...y] = a >= y
    #[logfn_inputs(TRACE)]
    pub fn greater_than(&self, other: &Self) -> Option<bool> {
        if self.is_bottom() || self.is_top() || other.is_bottom() || other.is_top() {
            None
        } else if self.lower_bound > other.upper_bound {
            Some(true)
        } else if other.lower_bound >= self.upper_bound {
            Some(false)
        } else {
            None
        }
    }

    // The expression that corresponds to this interval is not known to result in a integer value.
    // This is either because we just don't know, or because the necessary transfer function was
    // not implemented. The expectation is that bottom values will not often be encountered.
    // We don't need this domain to implement transfer functions for all operations that might
    // result in integer values since other domains will be preferred in those cases.
    #[logfn_inputs(TRACE)]
    pub fn is_bottom(&self) -> bool {
        self.upper_bound < self.lower_bound
    }

    // Returns true if this interval is known to be contained in the interval [target_type::MIN ... target_type::MAX].
    // A false result just means that we don't know, it never means that we know it does not.
    // Note that i128::MIN and i128::MAX are reserved to indicate missing (unbounded) lower and upper
    // bounds, respectively.
    #[logfn_inputs(TRACE)]
    pub fn is_contained_in(&self, target_type: ExpressionType) -> bool {
        if self.is_bottom() || self.is_top() {
            return false;
        };
        match target_type {
            I8 => {
                self.lower_bound >= i128::from(i8::MIN) && self.upper_bound <= i128::from(i8::MAX)
            }
            I16 => {
                self.lower_bound >= i128::from(i16::MIN) && self.upper_bound <= i128::from(i16::MAX)
            }
            I32 => {
                self.lower_bound >= i128::from(i32::MIN) && self.upper_bound <= i128::from(i32::MAX)
            }
            I64 => {
                self.lower_bound >= i128::from(i64::MIN) && self.upper_bound <= i128::from(i64::MAX)
            }
            I128 => self.lower_bound > i128::MIN && self.upper_bound < i128::MAX,
            Isize => {
                self.lower_bound >= (isize::MIN as i128) && self.upper_bound <= (isize::MAX as i128)
            }
            U8 => self.lower_bound >= 0 && self.upper_bound <= i128::from(u8::MAX),
            U16 => self.lower_bound >= 0 && self.upper_bound <= i128::from(u16::MAX),
            U32 => self.lower_bound >= 0 && self.upper_bound <= i128::from(u32::MAX),
            U64 => self.lower_bound >= 0 && self.upper_bound <= i128::from(u64::MAX),
            U128 => self.lower_bound >= 0 && self.upper_bound < i128::MAX,
            Usize => self.lower_bound >= 0 && self.upper_bound <= (usize::MAX as i128),
            _ => false,
        }
    }

    // Returns true if this interval is known to be contained in the interval [0 ... bit size of target_type).
    // A false result just means that we don't know, it never means that we know it does not.
    #[logfn_inputs(TRACE)]
    pub fn is_contained_in_width_of(&self, target_type: ExpressionType) -> bool {
        if self.is_bottom() || self.is_top() {
            return false;
        };
        match target_type {
            I8 | U8 => self.lower_bound >= 0 && self.upper_bound < 8,
            I16 | U16 => self.lower_bound >= 0 && self.upper_bound < 16,
            I32 | U32 => self.lower_bound >= 0 && self.upper_bound < 32,
            I64 | U64 => self.lower_bound >= 0 && self.upper_bound < 64,
            I128 | U128 => self.lower_bound >= 0 && self.upper_bound < 128,
            Isize | Usize => {
                self.lower_bound >= 0 && self.upper_bound < i128::from(usize::MAX.count_ones())
            }
            _ => false,
        }
    }

    // [x...y] intersect [a...b] = [max(x,a)...min(y,b)],
    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Self {
        if self.is_bottom() || other.is_bottom() {
            return BOTTOM.clone();
        }
        if self.is_top() {
            return other.clone();
        }
        if other.is_top() {
            return self.clone();
        }
        IntervalDomain {
            lower_bound: cmp::max(self.lower_bound, other.lower_bound),
            upper_bound: cmp::min(self.upper_bound, other.upper_bound),
        }
    }

    // All concrete integer values belong to this interval, so we know nothing.
    #[logfn_inputs(TRACE)]
    pub fn is_top(&self) -> bool {
        self.lower_bound == i128::MIN && self.upper_bound == i128::MAX
    }

    // [x...y] <= [a...b] = y <= a
    // !([x...y] <= [a...b]) = [a...b] < [x...y] = b < x
    #[logfn_inputs(TRACE)]
    pub fn less_equal(&self, other: &Self) -> Option<bool> {
        if self.is_bottom() || self.is_top() || other.is_bottom() || other.is_top() {
            None
        } else if self.upper_bound <= other.lower_bound {
            Some(true)
        } else if other.upper_bound < self.lower_bound {
            Some(false)
        } else {
            None
        }
    }

    // [x...y] < [a...b] = y < a
    // !([x...y] < [a...b]) = [a...b] <= [x...y] = b <= x
    #[logfn_inputs(TRACE)]
    pub fn less_than(&self, other: &Self) -> Option<bool> {
        if self.is_bottom() || self.is_top() || other.is_bottom() || other.is_top() {
            None
        } else if self.upper_bound < other.lower_bound {
            Some(true)
        } else if other.upper_bound <= self.lower_bound {
            Some(false)
        } else {
            None
        }
    }

    #[logfn_inputs(TRACE)]
    pub fn lower_bound(&self) -> Option<i128> {
        if self.lower_bound == TOP.lower_bound {
            None
        } else {
            Some(self.lower_bound)
        }
    }

    #[logfn_inputs(TRACE)]
    pub fn upper_bound(&self) -> Option<i128> {
        if self.upper_bound == TOP.upper_bound {
            None
        } else {
            Some(self.upper_bound)
        }
    }

    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn remove_lower_bound(&self) -> Self {
        IntervalDomain {
            lower_bound: TOP.lower_bound,
            upper_bound: self.upper_bound,
        }
    }

    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn remove_upper_bound(&self) -> Self {
        IntervalDomain {
            lower_bound: self.lower_bound,
            upper_bound: TOP.upper_bound,
        }
    }

    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn replace_upper_bound(&self, new_value: i128) -> Self {
        IntervalDomain {
            lower_bound: self.lower_bound,
            upper_bound: new_value,
        }
    }

    // [x,y] * [a,b] = [min(x*a, x*b, y*a, y*b), max(x*a, x*b, y*a, y*b)]
    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn mul(&self, other: &Self) -> Self {
        if self.is_bottom() || other.is_bottom() {
            return BOTTOM.clone();
        }
        if self.is_top() || other.is_top() {
            return TOP.clone();
        }
        let xa = self.lower_bound.saturating_mul(other.lower_bound);
        let xb = self.lower_bound.saturating_mul(other.upper_bound);
        let ya = self.upper_bound.saturating_mul(other.lower_bound);
        let yb = self.upper_bound.saturating_mul(other.upper_bound);
        IntervalDomain {
            lower_bound: xa.min(xb).min(ya).min(yb),
            upper_bound: xa.max(xb).max(ya).max(yb),
        }
    }

    // -[x...y] = [-y...-x]
    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn neg(&self) -> Self {
        if self.is_bottom() {
            return BOTTOM.clone();
        }
        if self.is_top() {
            return TOP.clone();
        }
        IntervalDomain {
            lower_bound: self.upper_bound.checked_neg().unwrap_or(i128::MAX),
            upper_bound: self.lower_bound.checked_neg().unwrap_or(i128::MAX),
        }
    }

    // [x...y] % [1...b] = [0...min(y, b-1)]
    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn rem(&self, other: &Self) -> Self {
        if self.is_bottom() || other.is_bottom() {
            return BOTTOM.clone();
        }
        if self.is_top() || other.is_top() {
            return TOP.clone();
        }
        if self.lower_bound >= 0 && other.lower_bound >= 1 {
            IntervalDomain {
                lower_bound: 0,
                upper_bound: i128::min(self.upper_bound, other.upper_bound - 1),
            }
        } else {
            TOP.clone()
        }
    }

    // [x...y] - [a...b] = [x-b...y-a]
    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn sub(&self, other: &Self) -> Self {
        if self.is_bottom() || other.is_bottom() {
            return BOTTOM.clone();
        }
        if self.is_top() || other.is_top() {
            return TOP.clone();
        }
        IntervalDomain {
            lower_bound: self.lower_bound.saturating_sub(other.upper_bound),
            upper_bound: self.upper_bound.saturating_sub(other.lower_bound),
        }
    }

    // [x...y] widen [a...b] = [min(x,a)...max(y,b)],
    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn widen(&self, other: &Self) -> Self {
        if self.is_bottom() || other.is_bottom() {
            return BOTTOM.clone();
        }
        if self.is_top() || other.is_top() {
            return TOP.clone();
        }
        IntervalDomain {
            lower_bound: cmp::min(self.lower_bound, other.lower_bound),
            upper_bound: cmp::max(self.upper_bound, other.upper_bound),
        }
    }
}
