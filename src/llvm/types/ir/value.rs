use std::{borrow::Cow, ffi::c_uchar, ptr::NonNull, slice};

use llvm_sys::{
    core::{
        LLVMGetNumOperands, LLVMGetOperand, LLVMGetValueName2, LLVMIsAFunction, LLVMIsAMDNode,
        LLVMIsAUser, LLVMPrintValueToString,
    },
    prelude::LLVMValueRef,
    LLVMValue,
};

use crate::llvm::{
    types::{
        ir::{
            function::Function,
            metadata::{MDNode, MetadataEntries},
        },
        LLVMTypeError, LLVMTypeWrapper,
    },
    Message,
};

/// LLVM Value Representation
///
/// This is a very important LLVM class. It is the base class of all values
/// computed by a program that may be used as operands to other values. Value is
/// the super class of other important classes such as Instruction and Function.
/// All Values have a Type. Type is not a subclass of Value. Some values can
/// have a name and they belong to some Module.  Setting the name on the Value
/// automatically updates the module's symbol table.
///
/// Every value has a "use list" that keeps track of which other Values are
/// using this Value.  A Value can also have an arbitrary number of ValueHandle
/// objects that watch it and listen to RAUW and Destroy events.  See
/// llvm/IR/ValueHandle.h for details.
#[derive(Clone)]
pub enum Value {
    MDNode(MDNode),
    Function(Function),
    Other(NonNull<LLVMValue>),
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value_to_string = |value| {
            Message {
                ptr: unsafe { LLVMPrintValueToString(value) },
            }
            .as_c_str()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
        };
        match self {
            Self::MDNode(node) => f
                .debug_struct("MDNode")
                .field("value", &value_to_string(node.as_ptr()))
                .finish(),
            Self::Function(fun) => f
                .debug_struct("Function")
                .field("value", &value_to_string(fun.as_ptr()))
                .finish(),
            Self::Other(value) => f
                .debug_struct("Other")
                .field("value", &value_to_string(value.as_ptr()))
                .finish(),
        }
    }
}

impl LLVMTypeWrapper for Value {
    type Target = LLVMValue;

    fn from_ptr(value_ref: NonNull<Self::Target>) -> Result<Self, LLVMTypeError> {
        if unsafe { !LLVMIsAMDNode(value_ref.as_ptr()).is_null() } {
            let mdnode = MDNode::from_ptr(value_ref)?;
            Ok(Value::MDNode(mdnode))
        } else if unsafe { !LLVMIsAFunction(value_ref.as_ptr()).is_null() } {
            Ok(Value::Function(Function::from_ptr(value_ref)?))
        } else {
            Ok(Value::Other(value_ref))
        }
    }

    fn as_non_null(&self) -> NonNull<Self::Target> {
        match self {
            Value::MDNode(mdnode) => mdnode.as_non_null(),
            Value::Function(f) => f.as_non_null(),
            Value::Other(value) => *value,
        }
    }

    fn as_ptr(&self) -> *mut Self::Target {
        match self {
            Value::MDNode(mdnode) => mdnode.as_ptr(),
            Value::Function(f) => f.as_ptr(),
            Value::Other(value) => value.as_ptr(),
        }
    }
}

impl Value {
    pub fn metadata_entries(&self) -> Option<MetadataEntries> {
        let value = match self {
            Value::MDNode(node) => node.as_ptr(),
            Value::Function(f) => f.as_ptr(),
            Value::Other(value) => value.as_ptr(),
        };
        MetadataEntries::new(NonNull::new(value).unwrap())
    }

    pub fn operands(&self) -> Option<impl Iterator<Item = LLVMValueRef>> {
        let value = match self {
            Value::MDNode(node) => Some(node.as_ptr()),
            Value::Function(f) => Some(f.as_ptr()),
            Value::Other(value) if unsafe { !LLVMIsAUser(value.as_ptr()).is_null() } => {
                Some(value.as_ptr())
            }
            _ => None,
        };

        value.map(|value| unsafe {
            (0..LLVMGetNumOperands(value)).map(move |i| LLVMGetOperand(value, i as u32))
        })
    }
}

pub trait NamedValue: LLVMTypeWrapper<Target = LLVMValue> {
    fn name<'a>(&self) -> Cow<'a, str> {
        let mut len = 0;
        let ptr = unsafe { LLVMGetValueName2(self.as_ptr(), &mut len) };
        let symbol_name = unsafe { slice::from_raw_parts(ptr as *const c_uchar, len) };
        String::from_utf8_lossy(symbol_name)
    }
}
