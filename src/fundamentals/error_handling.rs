//! Common error handling patterns for Rust coding interviews
//!
//! This module contains frequently-used patterns for Option, Result,
//! and error propagation.

#![allow(clippy::doc_markdown)] // Type names in docs are clear without backticks

/// Pattern: Option - unwrap_or for default values
#[must_use]
pub fn unwrap_or_example(opt: Option<i32>) -> i32 {
    opt.unwrap_or(0)
}

/// Pattern: Option - unwrap_or_else for computed defaults
#[must_use]
pub fn unwrap_or_else_example(opt: Option<i32>) -> i32 {
    opt.unwrap_or_else(|| 42)
}

/// Pattern: Option - map to transform inner value
#[must_use]
pub fn option_map_example(opt: Option<i32>) -> Option<i32> {
    opt.map(|x| x * 2)
}

/// Pattern: Option - and_then for chaining
#[must_use]
pub fn option_and_then_example(opt: Option<i32>) -> Option<i32> {
    opt.and_then(|x| if x > 0 { Some(x * 2) } else { None })
}

/// Pattern: Option - ok_or to convert to Result
#[must_use]
pub fn option_to_result(opt: Option<i32>) -> Result<i32, String> {
    opt.ok_or_else(|| "Value not found".to_string())
}

/// Pattern: Result - map for transforming success value
#[must_use]
pub fn result_map_example(res: Result<i32, String>) -> Result<i32, String> {
    res.map(|x| x * 2)
}

/// Pattern: Result - map_err for transforming error value
#[must_use]
pub fn result_map_err_example(res: Result<i32, String>) -> Result<i32, std::io::Error> {
    res.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

/// Pattern: Result - and_then for chaining operations
#[must_use]
pub fn result_and_then_example(res: Result<i32, String>) -> Result<i32, String> {
    res.and_then(|x| {
        if x > 0 {
            Ok(x * 2)
        } else {
            Err("Negative value".to_string())
        }
    })
}

/// Pattern: ? operator for early return on error
#[must_use]
pub fn question_mark_example(value: i32) -> Result<i32, String> {
    let checked = check_positive(value)?;
    let doubled = double_value(checked)?;
    Ok(doubled)
}

fn check_positive(value: i32) -> Result<i32, String> {
    if value > 0 {
        Ok(value)
    } else {
        Err("Value must be positive".to_string())
    }
}

fn double_value(value: i32) -> Result<i32, String> {
    Ok(value * 2)
}

/// Pattern: match on Option
#[must_use]
pub fn match_option_example(opt: Option<i32>) -> i32 {
    match opt {
        Some(x) => x * 2,
        None => 0,
    }
}

/// Pattern: match on Result
#[must_use]
pub fn match_result_example(res: Result<i32, String>) -> i32 {
    match res {
        Ok(x) => x,
        Err(_) => 0,
    }
}

/// Pattern: if let for Option
#[must_use]
pub fn if_let_option_example(opt: Option<i32>) -> i32 {
    if let Some(x) = opt { x * 2 } else { 0 }
}

/// Pattern: while let for iterating until empty
#[must_use]
pub fn while_let_example(mut values: Vec<Option<i32>>) -> Vec<i32> {
    let mut result = Vec::new();
    while let Some(opt_val) = values.pop() {
        if let Some(val) = opt_val {
            result.push(val);
        }
    }
    result
}

/// Pattern: collecting Results into Result<Vec<_>>
#[must_use]
pub fn collect_results(values: Vec<i32>) -> Result<Vec<i32>, String> {
    values
        .iter()
        .map(|&x| check_positive(x))
        .collect::<Result<Vec<_>, _>>()
}

/// Pattern: transpose Option<Result> to Result<Option>
#[must_use]
pub fn transpose_example(opt: Option<Result<i32, String>>) -> Result<Option<i32>, String> {
    opt.transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unwrap_or() {
        assert_eq!(unwrap_or_example(Some(5)), 5);
        assert_eq!(unwrap_or_example(None), 0);
    }

    #[test]
    fn test_unwrap_or_else() {
        assert_eq!(unwrap_or_else_example(Some(5)), 5);
        assert_eq!(unwrap_or_else_example(None), 42);
    }

    #[test]
    fn test_option_map() {
        assert_eq!(option_map_example(Some(5)), Some(10));
        assert_eq!(option_map_example(None), None);
    }

    #[test]
    fn test_option_and_then() {
        assert_eq!(option_and_then_example(Some(5)), Some(10));
        assert_eq!(option_and_then_example(Some(-5)), None);
        assert_eq!(option_and_then_example(None), None);
    }

    #[test]
    fn test_option_to_result() {
        assert!(option_to_result(Some(5)).is_ok());
        assert!(option_to_result(None).is_err());
    }

    #[test]
    fn test_result_map() {
        assert_eq!(result_map_example(Ok(5)), Ok(10));
        assert!(result_map_example(Err("error".to_string())).is_err());
    }

    #[test]
    fn test_result_and_then() {
        assert_eq!(result_and_then_example(Ok(5)), Ok(10));
        assert!(result_and_then_example(Ok(-5)).is_err());
    }

    #[test]
    fn test_question_mark() {
        assert_eq!(question_mark_example(5), Ok(10));
        assert!(question_mark_example(-5).is_err());
    }

    #[test]
    fn test_match_option() {
        assert_eq!(match_option_example(Some(5)), 10);
        assert_eq!(match_option_example(None), 0);
    }

    #[test]
    fn test_match_result() {
        assert_eq!(match_result_example(Ok(5)), 5);
        assert_eq!(match_result_example(Err("error".to_string())), 0);
    }

    #[test]
    fn test_if_let_option() {
        assert_eq!(if_let_option_example(Some(5)), 10);
        assert_eq!(if_let_option_example(None), 0);
    }

    #[test]
    fn test_while_let() {
        let values = vec![Some(1), Some(2), Some(3), None];
        assert_eq!(while_let_example(values), vec![3, 2, 1]);
    }

    #[test]
    fn test_collect_results() {
        assert_eq!(collect_results(vec![1, 2, 3]), Ok(vec![1, 2, 3]));
        assert!(collect_results(vec![1, -2, 3]).is_err());
    }

    #[test]
    fn test_transpose() {
        assert_eq!(transpose_example(Some(Ok(5))), Ok(Some(5)));
        assert_eq!(transpose_example(None), Ok(None));
        assert!(transpose_example(Some(Err("error".to_string()))).is_err());
    }
}
