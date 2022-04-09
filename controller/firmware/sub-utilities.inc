Pause1D:
    BANKSEL PAUSE_REG_0
    movff   PAUSE_REG_0, PAUSE_TMP_0
    
Pause1D_FirstLoop:
    decfsz  PAUSE_TMP_0
    goto    Pause1D_FirstLoop
    
    return
    
; uses PAUSE_REG_0 and PAUSE_REG_1 to set the intervals of each loop
; cycles = ((PAUSE_REG_0 * 3) - 1) + ?
Pause2D:
    BANKSEL PAUSE_REG_0
    movff   PAUSE_REG_0, PAUSE_TMP_0
    movff   PAUSE_REG_1, PAUSE_TMP_1
    
Pause2D_SecondLoop:
    movff   PAUSE_REG_0, PAUSE_TMP_0
    
Pause2D_FirstLoop:
    decfsz  PAUSE_TMP_0
    goto    Pause2D_FirstLoop
    
    decfsz  PAUSE_TMP_1
    goto    Pause2D_SecondLoop
    
    return
    
    
;;;;
Pause3D:
    BANKSEL PAUSE_REG_0
    movff   PAUSE_REG_0, PAUSE_TMP_0
    movff   PAUSE_REG_1, PAUSE_TMP_1
    movff   PAUSE_REG_2, PAUSE_TMP_2
    
Pause3D_ThirdLoop:
    movff   PAUSE_REG_1, PAUSE_TMP_1
    
Pause3D_SecondLoop:
    movff   PAUSE_REG_0, PAUSE_TMP_0
    
Pause3D_FirstLoop:
    decfsz  PAUSE_TMP_0
    goto    Pause3D_FirstLoop
    
    decfsz  PAUSE_TMP_1
    goto    Pause3D_SecondLoop
    
    decfsz  PAUSE_TMP_2
    goto    Pause3D_ThirdLoop
    
    return
    
    