// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn rejects_extra_arguments() {
    assert!(Top::try_parse_from(["top", "unexpected"]).is_err());
}

#[test]
fn linux_style_options_and_sampling_bounds() -> Result<(), Box<dyn std::error::Error>> {
    let args = Top::try_parse_from([
        "top",
        "-b",
        "-n",
        "1",
        "-d",
        "60",
        "-H",
        "-i",
        "-R",
        "-o",
        "time",
        "--normalize",
    ])?;
    assert!(args.batch && args.threads && args.idle_hide && args.reverse && args.normalize);
    assert_eq!(args.sort, Sort::Time);
    for delay in ["0", "NaN", "inf", "61", "-1"] {
        assert!(Top::try_parse_from(["top", "-d", delay]).is_err());
    }
    assert!(Top::try_parse_from(["top", "-n", "0"]).is_err());
    Ok(())
}
