    include "p18f47k42.inc"
    include "macros.inc"
    processor 18f47k42
    
    CONFIG WDTE = OFF
   ; CONFIG DEBUG = ON
    CONFIG LVP = ON
    CONFIG MCLRE = EXTMCLR
    CONFIG MVECEN = ON ; Enables Interrupt Vector Table ; IVTBASE + 2*(vector number)
    
    CONFIG RSTOSC = HFINTOSC_64MHZ
    CONFIG FEXTOSC = OFF
    CONFIG CLKOUTEN = OFF
    
    CONFIG XINST = ON
    
ResVec	    code    0x0000
    goto    Setup
    
U1RXVec	    code    0x003E  ; (0x0008 + (2 * 27))
    dw	    (0x0100>>2)
    
	    code	0x0600
; === Look at bottom of file for ISR routines ===
    
; === DEFINE PINS (text substitutions) ===
; refer to pinout documentation for more information
#define	    RELAY12	    LATA, 1
#define	    RELAY3	    LATA, 0

#define	    N64CNT4	    PORTC, 7
#define	    N64CNT3	    PORTC, 6
#define	    N64CNT2	    PORTC, 5
#define	    N64CNT1	    PORTC, 4

#define	    N64CNT4_LAT	    LATC, 7
#define	    N64CNT3_LAT	    LATC, 6
#define	    N64CNT2_LAT	    LATC, 5
#define	    N64CNT1_LAT	    LATC, 4

#define	    N64CNT4_TRIS    TRISC, 7
#define	    N64CNT3_TRIS    TRISC, 6
#define	    N64CNT2_TRIS    TRISC, 5
#define	    N64CNT1_TRIS    TRISC, 4




; === REGISTERS ===
; ACCESS BANK  (0x00 - 0x5F)
ZEROS_REG       equ H'00' ; Always 0x00
ONES_REG        equ H'01' ; Always 0xFF

; 0x02 - 0x0D unused

UTIL_FLAGS      equ H'0E' ; Utility Flags, initalized with 0x00
; <7:0> Unused

; Pause Clock
PAUSE_REG_0     equ H'10'
PAUSE_REG_1     equ H'11'
PAUSE_REG_2     equ H'12'

PAUSE_TMP_0     equ H'13'
PAUSE_TMP_1     equ H'14'
PAUSE_TMP_2     equ H'15'

; Auxillary Loop Counters
LOOP_COUNT_0    equ H'16'
LOOP_COUNT_1    equ H'17'


JUNK_REG        equ H'5F'

; BANK 0  (0x60 - 0xFF)

; BANK 1

; BANK 2



; === CONSTANT BYTES ===
USB_CMD_PING        equ H'01'
USB_CMD_ON	    equ H'02'
USB_CMD_OFF	    equ H'03'


; COMMON SUBROUTINES (may also contain macros) ;
    include "sub-utilities.inc"
    include "usb-handling.inc"
    
; Initialize device ;
Setup:
    include "startup.inc"
    
    movlb   B'000000'
    
;;;;;====================== Main Loop Start ======================;;;;;
Start:
    nop
    goto    Start
    
    
    
; INTERRUPT SUBROUTINES ;
    
U1RXISR	    code    0x0100
    BANKSEL PIR0
    bcf	    PIE3, U1RXIE
U1RXISR_Continue:
    movffl  U1RXB, WREG
    movlb   B'000000'
    movwf   JUNK_REG
    
    xorlw   USB_CMD_PING
    btfsc   STATUS, Z
    call    USBRX_Ping
    movf    JUNK_REG, 0
    
    xorlw   USB_CMD_ON
    btfsc   STATUS, Z
    call    USBRX_On
    movf    JUNK_REG, 0
    
    xorlw   USB_CMD_OFF
    btfsc   STATUS, Z
    call    USBRX_Off
    movf    JUNK_REG, 0
    
    BANKSEL PIR0
    btfsc   PIR3, U1RXIF
    goto    U1RXISR_Continue	; if buffer not empty, keep processing
    
    bsf	    PIE3, U1RXIE
    retfie
    
    
    end