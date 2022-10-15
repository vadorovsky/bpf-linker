use gimli::constants::*;

fn dw_tag_str_from_value_str(value_string: &str) -> Option<&str> {
    // note, this is currently a workaround because there is no official api to di this
    let start = value_string.find("tag: ")? + 5;
    let end = value_string[start..].find(",")? + start;
    Some(&value_string[start..end])
}

pub fn dw_tag_from_value_str(value_string: &str) -> Option<DwTag> {
    // note, this is currently a workaround because there is no official api to di this
    let tag = dw_tag_str_from_value_str(value_string)?;
    dw_tag_from_str(tag)
}

#[test]
fn test_dw_tag_str_from_value_str() {
    let input = "DICompositeType(tag: DW_TAG_structure_type, name: \"example\", scope: <0x13c61ef38>, file: <0x13c61bb60>, size: 8, align: 8, elements: <0x13c61f5e8>, templateParams: <0x13c61bc30>, identifier: \"e076b5316e99be834abb6515652cf749\")";
    assert!(dw_tag_str_from_value_str(input).eq(&Some("DW_TAG_structure_type")));
    assert!(dw_tag_str_from_value_str("tag: ,").eq(&Some("")));
    assert!(dw_tag_str_from_value_str("tag: ").eq(&None));
    assert!(dw_tag_str_from_value_str("tag:,").eq(&None));
    assert!(dw_tag_str_from_value_str(",").eq(&None));
    assert!(dw_tag_str_from_value_str(",tag:").eq(&None));
}

#[test]
fn test_dw_tag_from_value_str() {
    let input = "DICompositeType(tag: DW_TAG_structure_type, name: \"example\", scope: <0x13c61ef38>, file: <0x13c61bb60>, size: 8, align: 8, elements: <0x13c61f5e8>, templateParams: <0x13c61bc30>, identifier: \"e076b5316e99be834abb6515652cf749\")";
    assert!(dw_tag_from_value_str(input).eq(&Some(DW_TAG_structure_type)));
    assert!(dw_tag_from_value_str("tag: ,").eq(&None));
    assert!(dw_tag_from_value_str("tag: ").eq(&None));
    assert!(dw_tag_from_value_str("tag:,").eq(&None));
    assert!(dw_tag_from_value_str(",").eq(&None));
    assert!(dw_tag_from_value_str(",tag:").eq(&None));
}

fn dw_tag_from_str(tag: &str) -> Option<DwTag> {
    let result = match tag {
        "DW_TAG_null" => DW_TAG_null,
        "DW_TAG_array_type" => DW_TAG_array_type,
        "DW_TAG_class_type" => DW_TAG_class_type,
        "DW_TAG_entry_point" => DW_TAG_entry_point,
        "DW_TAG_enumeration_type" => DW_TAG_enumeration_type,
        "DW_TAG_formal_parameter" => DW_TAG_formal_parameter,
        "DW_TAG_imported_declaration" => DW_TAG_imported_declaration,
        "DW_TAG_label" => DW_TAG_label,
        "DW_TAG_lexical_block" => DW_TAG_lexical_block,
        "DW_TAG_member" => DW_TAG_member,
        "DW_TAG_pointer_type" => DW_TAG_pointer_type,
        "DW_TAG_reference_type" => DW_TAG_reference_type,
        "DW_TAG_compile_unit" => DW_TAG_compile_unit,
        "DW_TAG_string_type" => DW_TAG_string_type,
        "DW_TAG_structure_type" => DW_TAG_structure_type,
        "DW_TAG_subroutine_type" => DW_TAG_subroutine_type,
        "DW_TAG_typedef" => DW_TAG_typedef,
        "DW_TAG_union_type" => DW_TAG_union_type,
        "DW_TAG_unspecified_parameters" => DW_TAG_unspecified_parameters,
        "DW_TAG_variant" => DW_TAG_variant,
        "DW_TAG_common_block" => DW_TAG_common_block,
        "DW_TAG_common_inclusion" => DW_TAG_common_inclusion,
        "DW_TAG_inheritance" => DW_TAG_inheritance,
        "DW_TAG_inlined_subroutine" => DW_TAG_inlined_subroutine,
        "DW_TAG_module" => DW_TAG_module,
        "DW_TAG_ptr_to_member_type" => DW_TAG_ptr_to_member_type,
        "DW_TAG_set_type" => DW_TAG_set_type,
        "DW_TAG_subrange_type" => DW_TAG_subrange_type,
        "DW_TAG_with_stmt" => DW_TAG_with_stmt,
        "DW_TAG_access_declaration" => DW_TAG_access_declaration,
        "DW_TAG_base_type" => DW_TAG_base_type,
        "DW_TAG_catch_block" => DW_TAG_catch_block,
        "DW_TAG_const_type" => DW_TAG_const_type,
        "DW_TAG_constant" => DW_TAG_constant,
        "DW_TAG_enumerator" => DW_TAG_enumerator,
        "DW_TAG_file_type" => DW_TAG_file_type,
        "DW_TAG_friend" => DW_TAG_friend,
        "DW_TAG_namelist" => DW_TAG_namelist,
        "DW_TAG_namelist_item" => DW_TAG_namelist_item,
        "DW_TAG_packed_type" => DW_TAG_packed_type,
        "DW_TAG_subprogram" => DW_TAG_subprogram,
        "DW_TAG_template_type_parameter" => DW_TAG_template_type_parameter,
        "DW_TAG_template_value_parameter" => DW_TAG_template_value_parameter,
        "DW_TAG_thrown_type" => DW_TAG_thrown_type,
        "DW_TAG_try_block" => DW_TAG_try_block,
        "DW_TAG_variant_part" => DW_TAG_variant_part,
        "DW_TAG_variable" => DW_TAG_variable,
        "DW_TAG_volatile_type" => DW_TAG_volatile_type,

        // "DWARF 3.
        "DW_TAG_dwarf_procedure" => DW_TAG_dwarf_procedure,
        "DW_TAG_restrict_type" => DW_TAG_restrict_type,
        "DW_TAG_interface_type" => DW_TAG_interface_type,
        "DW_TAG_namespace" => DW_TAG_namespace,
        "DW_TAG_imported_module" => DW_TAG_imported_module,
        "DW_TAG_unspecified_type" => DW_TAG_unspecified_type,
        "DW_TAG_partial_unit" => DW_TAG_partial_unit,
        "DW_TAG_imported_unit" => DW_TAG_imported_unit,
        "DW_TAG_condition" => DW_TAG_condition,
        "DW_TAG_shared_type" => DW_TAG_shared_type,

        // "DWARF 4.
        "DW_TAG_type_unit" => DW_TAG_type_unit,
        "DW_TAG_rvalue_reference_type" => DW_TAG_rvalue_reference_type,
        "DW_TAG_template_alias" => DW_TAG_template_alias,

        // "DWARF 5.
        "DW_TAG_coarray_type" => DW_TAG_coarray_type,
        "DW_TAG_generic_subrange" => DW_TAG_generic_subrange,
        "DW_TAG_dynamic_type" => DW_TAG_dynamic_type,
        "DW_TAG_atomic_type" => DW_TAG_atomic_type,
        "DW_TAG_call_site" => DW_TAG_call_site,
        "DW_TAG_call_site_parameter" => DW_TAG_call_site_parameter,
        "DW_TAG_skeleton_unit" => DW_TAG_skeleton_unit,
        "DW_TAG_immutable_type" => DW_TAG_immutable_type,

        "DW_TAG_lo_user" => DW_TAG_lo_user,
        "DW_TAG_hi_user" => DW_TAG_hi_user,

        // SGI/MIPS extensions.
        "DW_TAG_MIPS_loop" => DW_TAG_MIPS_loop,

        // HP extensions.
        "DW_TAG_HP_array_descriptor" => DW_TAG_HP_array_descriptor,
        "DW_TAG_HP_Bliss_field" => DW_TAG_HP_Bliss_field,
        "DW_TAG_HP_Bliss_field_set" => DW_TAG_HP_Bliss_field_set,

        // GNU extensions.
        "DW_TAG_format_label" => DW_TAG_format_label,
        "DW_TAG_function_template" => DW_TAG_function_template,
        "DW_TAG_class_template" => DW_TAG_class_template,
        "DW_TAG_GNU_BINCL" => DW_TAG_GNU_BINCL,
        "DW_TAG_GNU_EINCL" => DW_TAG_GNU_EINCL,
        "DW_TAG_GNU_template_template_param" => DW_TAG_GNU_template_template_param,
        "DW_TAG_GNU_template_parameter_pack" => DW_TAG_GNU_template_parameter_pack,
        "DW_TAG_GNU_formal_parameter_pack" => DW_TAG_GNU_formal_parameter_pack,
        "DW_TAG_GNU_call_site" => DW_TAG_GNU_call_site,
        "DW_TAG_GNU_call_site_parameter" => DW_TAG_GNU_call_site_parameter,

        "DW_TAG_APPLE_property" => DW_TAG_APPLE_property,

        // SUN extensions.
        "DW_TAG_SUN_function_template" => DW_TAG_SUN_function_template,
        "DW_TAG_SUN_class_template" => DW_TAG_SUN_class_template,
        "DW_TAG_SUN_struct_template" => DW_TAG_SUN_struct_template,
        "DW_TAG_SUN_union_template" => DW_TAG_SUN_union_template,
        "DW_TAG_SUN_indirect_inheritance" => DW_TAG_SUN_indirect_inheritance,
        "DW_TAG_SUN_codeflags" => DW_TAG_SUN_codeflags,
        "DW_TAG_SUN_memop_info" => DW_TAG_SUN_memop_info,
        "DW_TAG_SUN_omp_child_func" => DW_TAG_SUN_omp_child_func,
        "DW_TAG_SUN_rtti_descriptor" => DW_TAG_SUN_rtti_descriptor,
        "DW_TAG_SUN_dtor_info" => DW_TAG_SUN_dtor_info,
        "DW_TAG_SUN_dtor" => DW_TAG_SUN_dtor,
        "DW_TAG_SUN_f90_interface" => DW_TAG_SUN_f90_interface,
        "DW_TAG_SUN_fortran_vax_structure" => DW_TAG_SUN_fortran_vax_structure,

        // ALTIUM extensions.
        "DW_TAG_ALTIUM_circ_type" => DW_TAG_ALTIUM_circ_type,
        "DW_TAG_ALTIUM_mwa_circ_type" => DW_TAG_ALTIUM_mwa_circ_type,
        "DW_TAG_ALTIUM_rev_carry_type" => DW_TAG_ALTIUM_rev_carry_type,
        "DW_TAG_ALTIUM_rom" => DW_TAG_ALTIUM_rom,

        // Extensions for UPC.
        "DW_TAG_upc_shared_type" => DW_TAG_upc_shared_type,
        "DW_TAG_upc_strict_type" => DW_TAG_upc_strict_type,
        "DW_TAG_upc_relaxed_type" => DW_TAG_upc_relaxed_type,

        // PGI (STMicroelectronics) extensions.
        "DW_TAG_PGI_kanji_type" => DW_TAG_PGI_kanji_type,
        "DW_TAG_PGI_interface_block" => DW_TAG_PGI_interface_block,

        // Borland extensions.
        "DW_TAG_BORLAND_property" => DW_TAG_BORLAND_property,
        "DW_TAG_BORLAND_Delphi_string" => DW_TAG_BORLAND_Delphi_string,
        "DW_TAG_BORLAND_Delphi_dynamic_array" => DW_TAG_BORLAND_Delphi_dynamic_array,
        "DW_TAG_BORLAND_Delphi_set" => DW_TAG_BORLAND_Delphi_set,
        "DW_TAG_BORLAND_Delphi_variant" => DW_TAG_BORLAND_Delphi_variant,
        _ => return None,
    };
    Some(result)
}
