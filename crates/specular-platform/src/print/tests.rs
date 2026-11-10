use super::*;

#[test]
fn borderless_is_read_from_the_printer_options_never_guessed() {
    // A driverless (IPP Everywhere) printer: borderless sizes are their own entries.
    let ipp = "PageSize/Media Size: Letter Letter.Borderless *A4 A4.Borderless 4x6.Borderless\n\
               ColorModel/Output Mode: *RGB Gray\n";
    assert_eq!(borderless_from_lpoptions(ipp), Borderless::Yes);
    // Gutenprint-style full bleed sizes and a driver switch.
    assert_eq!(
        borderless_from_lpoptions("PageSize/Media Size: *A4 A4.FB Letter\n"),
        Borderless::Yes
    );
    assert_eq!(
        borderless_from_lpoptions("PageSize/Size: *A4\nBorderless/Borderless: *False True\n"),
        Borderless::Yes
    );
    // Paper sizes listed, none borderless.
    assert_eq!(
        borderless_from_lpoptions("PageSize/Media Size: *A4 Letter Legal\nDuplex/2-Sided: *None\n"),
        Borderless::No
    );
    // Nothing about paper: not known.
    assert_eq!(borderless_from_lpoptions(""), Borderless::Unknown);
    assert_eq!(
        borderless_from_lpoptions("ColorModel/Output Mode: *RGB\n"),
        Borderless::Unknown
    );
    assert_eq!(
        borderless_from_lpoptions("Borderless/Borderless: *False\nPageSize/Size: *A4\n"),
        Borderless::No
    );
}
