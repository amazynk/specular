//! The codes of ISO 15740 (PTP 1.0/1.1) used here: container types, operations, responses,
//! events, device properties, data types and object formats.

/// Container types (the `Type` field of every USB container).
pub mod container {
    pub const COMMAND: u16 = 1;
    pub const DATA: u16 = 2;
    pub const RESPONSE: u16 = 3;
    pub const EVENT: u16 = 4;
}

/// Operation codes.
pub mod op {
    pub const GET_DEVICE_INFO: u16 = 0x1001;
    pub const OPEN_SESSION: u16 = 0x1002;
    pub const CLOSE_SESSION: u16 = 0x1003;
    pub const GET_STORAGE_IDS: u16 = 0x1004;
    pub const GET_STORAGE_INFO: u16 = 0x1005;
    pub const GET_NUM_OBJECTS: u16 = 0x1006;
    pub const GET_OBJECT_HANDLES: u16 = 0x1007;
    pub const GET_OBJECT_INFO: u16 = 0x1008;
    pub const GET_OBJECT: u16 = 0x1009;
    pub const GET_THUMB: u16 = 0x100A;
    pub const DELETE_OBJECT: u16 = 0x100B;
    pub const INITIATE_CAPTURE: u16 = 0x100E;
    pub const GET_DEVICE_PROP_DESC: u16 = 0x1014;
    pub const GET_DEVICE_PROP_VALUE: u16 = 0x1015;
    pub const SET_DEVICE_PROP_VALUE: u16 = 0x1016;
    pub const GET_PARTIAL_OBJECT: u16 = 0x101B;
}

/// Response codes.
pub mod rc {
    pub const OK: u16 = 0x2001;
    pub const GENERAL_ERROR: u16 = 0x2002;
    pub const SESSION_NOT_OPEN: u16 = 0x2003;
    pub const INVALID_TRANSACTION_ID: u16 = 0x2004;
    pub const OPERATION_NOT_SUPPORTED: u16 = 0x2005;
    pub const PARAMETER_NOT_SUPPORTED: u16 = 0x2006;
    pub const INCOMPLETE_TRANSFER: u16 = 0x2007;
    pub const INVALID_STORAGE_ID: u16 = 0x2008;
    pub const INVALID_OBJECT_HANDLE: u16 = 0x2009;
    pub const DEVICE_PROP_NOT_SUPPORTED: u16 = 0x200A;
    pub const STORE_FULL: u16 = 0x200C;
    pub const STORE_READ_ONLY: u16 = 0x200E;
    pub const ACCESS_DENIED: u16 = 0x200F;
    pub const STORE_NOT_AVAILABLE: u16 = 0x2013;
    pub const DEVICE_BUSY: u16 = 0x2019;
    pub const INVALID_DEVICE_PROP_FORMAT: u16 = 0x201B;
    pub const INVALID_DEVICE_PROP_VALUE: u16 = 0x201C;
    pub const INVALID_PARAMETER: u16 = 0x201D;
    pub const SESSION_ALREADY_OPEN: u16 = 0x201E;
    pub const TRANSACTION_CANCELLED: u16 = 0x201F;

    /// The response's name in the standard, for logs (`None` for a vendor or unknown code).
    pub fn name(code: u16) -> Option<&'static str> {
        Some(match code {
            0x2001 => "OK",
            0x2002 => "General_Error",
            0x2003 => "Session_Not_Open",
            0x2004 => "Invalid_TransactionID",
            0x2005 => "Operation_Not_Supported",
            0x2006 => "Parameter_Not_Supported",
            0x2007 => "Incomplete_Transfer",
            0x2008 => "Invalid_StorageID",
            0x2009 => "Invalid_ObjectHandle",
            0x200A => "DeviceProp_Not_Supported",
            0x200B => "Invalid_ObjectFormatCode",
            0x200C => "Store_Full",
            0x200D => "Object_WriteProtected",
            0x200E => "Store_Read-Only",
            0x200F => "Access_Denied",
            0x2010 => "No_Thumbnail_Present",
            0x2011 => "SelfTest_Failed",
            0x2012 => "Partial_Deletion",
            0x2013 => "Store_Not_Available",
            0x2014 => "Specification_By_Format_Unsupported",
            0x2015 => "No_Valid_ObjectInfo",
            0x2016 => "Invalid_Code_Format",
            0x2017 => "Unknown_Vendor_Code",
            0x2018 => "Capture_Already_Terminated",
            0x2019 => "Device_Busy",
            0x201A => "Invalid_ParentObject",
            0x201B => "Invalid_DeviceProp_Format",
            0x201C => "Invalid_DeviceProp_Value",
            0x201D => "Invalid_Parameter",
            0x201E => "Session_Already_Open",
            0x201F => "Transaction_Cancelled",
            0x2020 => "Specification_of_Destination_Unsupported",
            _ => return None,
        })
    }
}

/// Event codes.
pub mod ev {
    pub const CANCEL_TRANSACTION: u16 = 0x4001;
    pub const OBJECT_ADDED: u16 = 0x4002;
    pub const OBJECT_REMOVED: u16 = 0x4003;
    pub const STORE_ADDED: u16 = 0x4004;
    pub const STORE_REMOVED: u16 = 0x4005;
    pub const DEVICE_PROP_CHANGED: u16 = 0x4006;
    pub const OBJECT_INFO_CHANGED: u16 = 0x4007;
    pub const DEVICE_INFO_CHANGED: u16 = 0x4008;
    pub const REQUEST_OBJECT_TRANSFER: u16 = 0x4009;
    pub const STORE_FULL: u16 = 0x400A;
    pub const DEVICE_RESET: u16 = 0x400B;
    pub const STORAGE_INFO_CHANGED: u16 = 0x400C;
    pub const CAPTURE_COMPLETE: u16 = 0x400D;
}

/// Device property codes.
pub mod prop {
    pub const BATTERY_LEVEL: u16 = 0x5001;
    pub const WHITE_BALANCE: u16 = 0x5005;
    pub const F_NUMBER: u16 = 0x5007;
    pub const FOCAL_LENGTH: u16 = 0x5008;
    pub const EXPOSURE_TIME: u16 = 0x500D;
    pub const EXPOSURE_PROGRAM_MODE: u16 = 0x500E;
    pub const EXPOSURE_INDEX: u16 = 0x500F;
    pub const EXPOSURE_BIAS_COMPENSATION: u16 = 0x5010;
}

/// Data type codes of property values.
pub mod ty {
    pub const INT8: u16 = 0x0001;
    pub const UINT8: u16 = 0x0002;
    pub const INT16: u16 = 0x0003;
    pub const UINT16: u16 = 0x0004;
    pub const INT32: u16 = 0x0005;
    pub const UINT32: u16 = 0x0006;
    pub const INT64: u16 = 0x0007;
    pub const UINT64: u16 = 0x0008;
    pub const INT128: u16 = 0x0009;
    pub const UINT128: u16 = 0x000A;
    /// Arrays are the element type with this bit set (`AINT8` = 0x4001 …).
    pub const ARRAY: u16 = 0x4000;
    pub const STR: u16 = 0xFFFF;
}

/// Object format codes.
pub mod format {
    pub const UNDEFINED: u16 = 0x3000;
    /// A folder.
    pub const ASSOCIATION: u16 = 0x3001;
    pub const EXIF_JPEG: u16 = 0x3801;
}

/// The storage id meaning every storage, and the parent meaning any.
pub const ALL: u32 = 0xFFFF_FFFF;
