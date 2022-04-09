    movlb   B'00000000' ; sets current GPR bank to bank 0
    
    lfsr    2, 0x00 ; sets access bank start location to 0x00
    
    
    ; === Peripheral Pin Select ===
    movlb   B'111010'   ; Bank 58
    movlw   B'010011'
    movwf   RC1PPS      ; Set U1TX      to pin RC1
    
    movlw   B'010000'
    movwf   U1RXPPS     ; Set U1RX      to pin RC0
    
    movlb   B'00000000'
    ; === Register Setup ===
    clrf    ZEROS_REG
    setf    ONES_REG
    clrf    UTIL_FLAGS
    
    ; configure I/O ports ; refer to pinout spreadsheet/docs for how these are mapped
    
    ; enable digitial input buffers
    BANKSEL ANSELA
    clrf    ANSELA
    clrf    ANSELB
    clrf    ANSELC
    clrf    ANSELD
    clrf    ANSELE
    
    BANKSEL ZEROS_REG
    clrf    LATA
    clrf    LATB
    clrf    LATC
    clrf    LATD
    clrf    LATE
    
    ; 0 is output, 1 is input
    BANKSEL TRISA
    movlw   B'11111100'
    movwf   TRISA
    
    movlw   B'11111111'
    movwf   TRISB
    
    movlw   B'11111101'
    movwf   TRISC
    
    movlw   B'11111111'
    movwf   TRISD
    
    movlw   B'11111111'
    movwf   TRISE
    
    BANKSEL SLRCONA
    clrf    SLRCONA
    clrf    SLRCONB
    clrf    SLRCONC
    clrf    SLRCOND
    clrf    SLRCONE
    
    BANKSEL WPUC
    bsf	    WPUC, 7
    bsf	    WPUC, 6
    bsf	    WPUC, 5
    bsf	    WPUC, 4
    
    BANKSEL ZEROS_REG
    clrf    LATA
    clrf    LATB
    clrf    LATC
    clrf    LATD
    clrf    LATE
    
    ; === Enable UART ===
    BANKSEL U1CON0
    bsf     U1CON0, U1TXEN  ; enable TX
    bsf     U1CON0, U1RXEN  ; enable RX
                            ; MODE is 0000 by default, which sets UART to Async 8-bit
    bcf     U1CON0, U1BRGS  ; normal baud rate formula
    clrf    U1BRGH
    movlw   D'7'
    movwf   U1BRGL          ; set baud rate to 500,000
    bsf     U1CON1, U1ON    ; enable UART1
    
    wait D'16'
    
    
    ; === Enable Interrupts ===
    BANKSEL PIR0
    bcf	    PIR3, U1RXIF
    bsf	    PIE3, U1RXIE
    BANKSEL INTCON0
    bcf	    INTCON0, IPEN_INTCON0   ; Priority is unnecessary, make sure it's left off
    bsf	    INTCON0, GIE    ; Global Interrupt Enable bit
    
    
    
    
    
    
    
    ; ====== End ======