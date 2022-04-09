wait macro cycles
    local three = cycles / 3
    local two = (cycles % 3) / 2
    local one = (cycles % 3) % 2

    while three > 0
        goto $+2
three--
    endw

    if two
        goto $+4
    endif

    if one
        nop
    endif
    endm
    
    
writeLitAddrData macro addr_upper, addr_lower, value
    movlw   addr_upper
    movwf   ADDR_BUS_UPPER_LAT
    movlw   addr_lower
    movwf   ADDR_BUS_LOWER_LAT
    bcf	    PIN_WE
    movlw   value
    movwf   DATA_BUS_LAT
    bsf	    PIN_WE
    
    endm
    
writeProgAddrData macro
    movffl  PROG_ADDR_A16, ADDR_BUS_A16_LAT
    movffl  PROG_ADDR_UPPER, ADDR_BUS_UPPER_LAT
    movffl  PROG_ADDR_LOWER, ADDR_BUS_LOWER_LAT
    bcf	    PIN_WE
    nop
    movffl  PROG_BYTE, DATA_BUS_LAT
    bsf	    PIN_WE
    
    endm