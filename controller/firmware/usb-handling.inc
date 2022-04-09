    ;;;;==================================================================;;;;
USBRX_Ping: ; 0x01
    movlw   H'E1'
    movffl  WREG, U1TXB
    return
    
    ;;;;==================================================================;;;;
USBRX_On: ; 0x02
    bsf	    RELAY12
    bsf	    RELAY3
    movlw   H'E2'
    movffl  WREG, U1TXB
    return
    
    ;;;;==================================================================;;;;
    
USBRX_Off: ; 0x03
    bcf	    RELAY12
    bcf	    RELAY3
    movlw   H'E3'
    movffl  WREG, U1TXB
    return
    
    ;;;;==================================================================;;;;
    
    