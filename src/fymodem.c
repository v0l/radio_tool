/**
 * Free YModem implementation.
 *
 * Fredrik Hederstierna 2014
 *
 * This file is in the public domain.
 * You can do whatever you want with it.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
 */

#include <fymodem.h>
#include <stdio.h>

#ifndef _WIN32
#include <termios.h>
#include <unistd.h>
#else
#define WIN32_LEAN_AND_MEAN
#include <Windows.h>
#endif

/* filesize 999999999999999 should be enough... */
#define YM_FILE_SIZE_LENGTH        (16)

/* packet constants */
#define YM_PACKET_SEQNO_INDEX      (1)
#define YM_PACKET_SEQNO_COMP_INDEX (2)
#define YM_PACKET_HEADER           (3)      /* start, block, block-complement */
#define YM_PACKET_TRAILER          (2)      /* CRC bytes */
#define YM_PACKET_OVERHEAD         (YM_PACKET_HEADER + YM_PACKET_TRAILER)
#define YM_PACKET_SIZE             (128)
#define YM_PACKET_1K_SIZE          (1024)
#define YM_PACKET_RX_TIMEOUT_MS    (2000)
#define YM_PACKET_ERROR_MAX_NBR    (5)

/* contants defined by YModem protocol */
#define YM_SOH                     (0x01)  /* start of 128-byte data packet */
#define YM_STX                     (0x02)  /* start of 1024-byte data packet */
#define YM_EOT                     (0x04)  /* End Of Transmission */
#define YM_ACK                     (0x06)  /* ACKnowledge, receive OK */
#define YM_NAK                     (0x15)  /* Negative ACKnowledge, receiver ERROR, retry */
#define YM_CAN                     (0x18)  /* two CAN in succession will abort transfer */
#define YM_CRC                     (0x43)  /* 'C' == 0x43, request 16-bit CRC, use in place of first NAK for CRC mode */
#define YM_ABT1                    (0x41)  /* 'A' == 0x41, assume try abort by user typing */
#define YM_ABT2                    (0x61)  /* 'a' == 0x61, assume try abort by user typing */

/* ------------------------------------------------ */

/* error logging function */
#define YM_ERR(fmt, ...) do { printf(fmt, __VA_ARGS__); } while(0)

FILE* global_fd = 0;

char __ym_getchar(int timeout_ms)
{
    char c = 0;
    fread(&c, 1, 1, global_fd);
    return c;
}

void __ym_putchar(char c)
{
    fwrite(&c, 1, 1, global_fd);
}

void __ym_sleep_ms(int delay_ms)
{
#ifdef _WIN32
    Sleep(delay_ms);
#else
    usleep(delay_ms * 1000);
#endif
}

void __ym_flush()
{
#ifndef _WIN32
    tcflush(global_fd,TCIOFLUSH);
#endif
}

/* ------------------------------------------------ */
/* calculate crc16-ccitt very fast
   Idea from: http://www.ccsinfo.com/forum/viewtopic.php?t=24977
*/
static uint16_t ym_crc16(const uint8_t *buf, uint16_t len) 
{
  uint16_t x;
  uint16_t crc = 0;
  while (len--) {
    x = (crc >> 8) ^ *buf++;
    x ^= x >> 4;
    crc = (crc << 8) ^ (x << 12) ^ (x << 5) ^ x;
  }
  return crc;
}

/* ------------------------------------------------- */
/* write 32bit value as asc to buffer, return chars written. */
static uint32_t ym_writeU32(uint32_t val, uint8_t *buf)
{
  uint32_t ci = 0;
  if (val == 0) {
    /* If already zero then just return zero */
    buf[ci++] = '0';
  }
  else {
    /* Maximum number of decimal digits in uint32_t is 10, add one for z-term */
    uint8_t s[11];
    int32_t i = sizeof(s) - 1;
    /* z-terminate string */
    s[i] = 0;
    while ((val > 0) && (i > 0)) {
      /* write decimal char */
      s[--i] = (val % 10) + '0';
      val /= 10;
    }
    uint8_t *sp = &s[i];
    /* copy results to out buffer */
    while (*sp) {
      buf[ci++] = *sp++;
    }
  }
  /* z-term */
  buf[ci] = 0;
  /* return chars written */
  return ci;
}

/* ------------------------------------------------- */
/* read 32bit asc value from buffer */
static void ym_readU32(const uint8_t* buf, uint32_t *val)
{
  const uint8_t *s = buf;
  uint32_t res = 0;
  uint8_t c;  
  /* trim and strip leading spaces if any */
  do {
    c = *s++;
  } while (c == ' ');
  while ((c >= '0') && (c <= '9')) {
    c -= '0';
    res *= 10;
    res += c;
    /* next char */
    c = *s++;
  }
  *val = res;
}

/* -------------------------------------------------- */
/**
  * Receive a packet from sender
  * @param rxlen
  *     0: end of transmission
  *    -1: abort by sender
  *    >0: packet length
  * @return 0: normally return, success
  *        -1: timeout or packet error
  *         1: abort by user / corrupt packet
  */
static int32_t ym_rx_packet(uint8_t *rxdata,
                            int32_t *rxlen,
                            uint32_t packets_rxed,
                            uint32_t timeout_ms)
{
  *rxlen = 0;
  
  int32_t c = __ym_getchar(timeout_ms);
  if (c < 0) {
    /* end of stream */
    return -1;
  }

  uint32_t rx_packet_size;

  switch (c) {
  case YM_SOH:
    rx_packet_size = YM_PACKET_SIZE;
    break;
  case YM_STX:
    rx_packet_size = YM_PACKET_1K_SIZE;
    break;
  case YM_EOT:
    /* ok */
    return 0;
  case YM_CAN:
    c = __ym_getchar(timeout_ms);
    if (c == YM_CAN) {
      *rxlen = -1;
      /* ok */
      return 0;
    }
    /* fall-through */
  case YM_CRC:
    if (packets_rxed == 0) {
      /* could be start condition, first byte */
      return 1;
    }
   /* fall-through */
  case YM_ABT1:
  case YM_ABT2:
    /* User try abort, 'A' or 'a' received */
    return 1;
  default:
    /* This case could be the result of corruption on the first octet
       of the packet, but it's more likely that it's the user banging
       on the terminal trying to abort a transfer. Technically, the
       former case deserves a NAK, but for now we'll just treat this
       as an abort case. */
    *rxlen = -1;
    return 0;
  }
  
  /* store data RXed */
  *rxdata = (uint8_t)c;
  
  uint32_t i;
  for (i = 1; i < (rx_packet_size + YM_PACKET_OVERHEAD); i++) {
    c = __ym_getchar(timeout_ms);
    if (c < 0) {
      /* end of stream */
      return -1;
    }
    /* store data RXed */
    rxdata[i] = (uint8_t)c;
  }
  
  /* just a sanity check on the sequence number/complement value.
     caller should check for in-order arrival. */
  uint8_t seq_nbr = (rxdata[YM_PACKET_SEQNO_INDEX] & 0xff);
  uint8_t seq_cmp = ((rxdata[YM_PACKET_SEQNO_COMP_INDEX] ^ 0xff) & 0xff);
  if (seq_nbr != seq_cmp) {
    /* seq nbr error */
    return 1;
  }
  
  /* check CRC16 match */
  uint16_t crc16_val = ym_crc16((const unsigned char *)(rxdata + YM_PACKET_HEADER),
                                rx_packet_size + YM_PACKET_TRAILER);
  if (crc16_val) {
    /* CRC error, non zero */
    return 1;
  }
  *rxlen = rx_packet_size;
  /* success */
  return 0;
}

/* ------------------------------------------------- */
/**
 * Receive a file using the ymodem protocol
 * @param rxdata Pointer to the first byte
 * @param rxlen  Max in length
 * @return The length of the file received, or 0 on error
 */
int32_t fymodem_receive(uint8_t *rxdata,
                        size_t rxlen,
                        char filename[FYMODEM_FILE_NAME_MAX_LENGTH])
{
  /* alloc 1k on stack, ok? */
  uint8_t rx_packet_data[YM_PACKET_1K_SIZE + YM_PACKET_OVERHEAD];
  int32_t rx_packet_len;

  uint8_t filesize_asc[YM_FILE_SIZE_LENGTH];
  uint32_t filesize = 0;

  bool first_try = true;
  bool session_done = false;

  uint32_t nbr_errors = 0;

  /* z-term string */
  filename[0] = 0;
  
  /* receive files */
  do { /* ! session done */
    if (first_try) {
      /* initiate transfer */
      __ym_putchar(YM_CRC);
    }
    first_try = false;

    bool crc_nak = true;
    bool file_done = false;
    uint32_t packets_rxed = 0;

    /* set start position of rxing data */
    uint8_t *rxptr = rxdata;
    do { /* ! file_done */
      /* receive packets */
      int32_t res = ym_rx_packet(rx_packet_data,
                                 &rx_packet_len,
                                 packets_rxed,
                                 YM_PACKET_RX_TIMEOUT_MS);
      switch (res) {
      case 0: {
        /* packet received, clear packet error counter */
        nbr_errors = 0;
        switch (rx_packet_len) {
        case -1: {
          /* aborted by sender */
          __ym_putchar(YM_ACK);
          return 0;
        }
        case 0: {
          /* EOT - End Of Transmission */
          __ym_putchar(YM_ACK);
          /* TODO: Add some sort of sanity check on the number of
             packets received and the advertised file length. */
          file_done = true;
          /* resend CRC to re-initiate transfer */
          __ym_putchar(YM_CRC);
          break;
        }
        default: {
          /* normal packet, check seq nbr */
          uint8_t seq_nbr = rx_packet_data[YM_PACKET_SEQNO_INDEX];
          if (seq_nbr != (packets_rxed & 0xff)) {
            /* wrong seq number */
            __ym_putchar(YM_NAK);
          } else {
            if (packets_rxed == 0) {
              /* The spec suggests that the whole data section should
                 be zeroed, but some senders might not do this.
                 If we have a NULL filename and the first few digits of
                 the file length are zero, then call it empty. */
              int32_t i;
              for (i = YM_PACKET_HEADER; i < YM_PACKET_HEADER + 4; i++) {
                if (rx_packet_data[i] != 0) {
                  break;
                }
              }
              /* non-zero bytes found in header, filename packet has data */
              if (i < YM_PACKET_HEADER + 4) {
                /* read file name */
                uint8_t *file_ptr = (uint8_t*)(rx_packet_data + YM_PACKET_HEADER);
                i = 0;
                while ((*file_ptr != '\0') &&
                       (i < FYMODEM_FILE_NAME_MAX_LENGTH)) {
                  filename[i++] = *file_ptr++;
                }
                filename[i++] = '\0';
                /* skip null term char */
                file_ptr++;
                /* read file size */
                i = 0;
                while ((*file_ptr != '\0') &&
                       (*file_ptr != ' ')  &&
                       (i < YM_FILE_SIZE_LENGTH)) {
                  filesize_asc[i++] = *file_ptr++;
                }
                filesize_asc[i++] = '\0';
                /* convert file size */
                ym_readU32(filesize_asc, &filesize);
                /* check file size */
                if (filesize > rxlen) {
                  YM_ERR("YM: RX buffer too small (0x%08x vs 0x%08x)\n", (unsigned int)rxlen, (unsigned int)filesize);
                  goto rx_err_handler;
                }
                __ym_putchar(YM_ACK);
                __ym_putchar(crc_nak ? YM_CRC : YM_NAK);
                crc_nak = false;
              }
              else {
                /* filename packet is empty, end session */
                __ym_putchar(YM_ACK);
                file_done = true;
                session_done = true;
                break;
              }
            }
            else {
              /* This shouldn't happen, but we check anyway in case the
                 sender sent wrong info in its filename packet */
              if (((rxptr + rx_packet_len) - rxdata) > (int32_t)rxlen) {
                YM_ERR("YM: RX buffer overflow (exceeded 0x%08x)\n", (unsigned int)rxlen);
                goto rx_err_handler;
              }
              int32_t i;
              for (i = 0; i < rx_packet_len; i++) {
                rxptr[i] = rx_packet_data[YM_PACKET_HEADER + i];
              }
              rxptr += rx_packet_len;
              __ym_putchar(YM_ACK);
            }
            packets_rxed++;
          }  /* sequence number check ok */
        } /* default */
        } /* inner switch */
        break;
      } /* case 0 */
      default: {
        /* ym_rx_packet() returned error */
        if (packets_rxed > 0) {
          nbr_errors++;
          if (nbr_errors >= YM_PACKET_ERROR_MAX_NBR) {
            YM_ERR("YM: RX errors too many: %d - ABORT.\n", (unsigned int)nbr_errors);
            goto rx_err_handler;
          }
        }
        __ym_putchar(YM_CRC);
        break;
      } /* default */
      } /* switch */
      
      /* check end of receive packets */
    } while (! file_done);

    /* check end of receive files */
  } while (! session_done);

  /* return bytes received */
  return filesize;

 rx_err_handler:
  __ym_putchar(YM_CAN);
  __ym_putchar(YM_CAN);
  __ym_sleep_ms(1000);
  return 0;
}

/* ------------------------------------ */
static void ym_send_packet(uint8_t *txdata,
                           int32_t block_nbr)
{
  int32_t tx_packet_size;

  /* We use a short packet for block 0, all others are 1K */
  if (block_nbr == 0) {
    tx_packet_size = YM_PACKET_SIZE;
  }
  else {
    tx_packet_size = YM_PACKET_1K_SIZE;
  }

  uint16_t crc16_val = ym_crc16(txdata, tx_packet_size);
  
  /* For 128 byte packets use SOH, for 1K use STX */
  __ym_putchar( (block_nbr == 0) ? YM_SOH : YM_STX );
  /* write seq numbers */
  __ym_putchar(block_nbr & 0xFF);
  __ym_putchar(~block_nbr & 0xFF);
  
  /* write txdata */
  int32_t i;
  for (i = 0; i < tx_packet_size; i++) {
    __ym_putchar(txdata[i]);
  }

  /* write crc16 */
  __ym_putchar((crc16_val >> 8) & 0xFF);
  __ym_putchar(crc16_val & 0xFF);
}

/* ----------------------------------------------- */
/* Send block 0 (the filename block), filename might be truncated to fit. */
static void ym_send_packet0(const char* filename,
                            int32_t filesize)
{
  int32_t pos = 0;
  /* put 256byte on stack, ok? reuse other stack mem? */
  uint8_t block[YM_PACKET_SIZE];
  if (filename) {
    /* write filename */
    while (*filename && (pos < YM_PACKET_SIZE - YM_FILE_SIZE_LENGTH - 2)) {
      block[pos++] = *filename++;
    }
    /* z-term filename */
    block[pos++] = 0;
    
    /* write size, TODO: check if buffer can overwritten here. */
    pos += ym_writeU32(filesize, &block[pos]);
  }

  /* z-terminate string, pad with zeros */
  while (pos < YM_PACKET_SIZE) {
    block[pos++] = 0;
  }
  
  /* send header block */
  ym_send_packet(block, 0);
}

/* ------------------------------------------------- */
static void ym_send_data_packets(uint8_t* txdata,
                                 uint32_t txlen,
                                 uint32_t timeout_ms)
{
  int32_t block_nbr = 1;
  
  while (txlen > 0) {
    /* check if send full 1k packet */
    uint32_t send_size;
    if (txlen > YM_PACKET_1K_SIZE) {
      send_size = YM_PACKET_1K_SIZE;
    } else {
      send_size = txlen;
    }
    /* send packet */
    ym_send_packet(txdata, block_nbr);
    int32_t c = __ym_getchar(timeout_ms);
    switch (c) {
    case YM_ACK: {
      txdata += send_size;
      txlen  -= send_size;
      block_nbr++;
      break;
    }
    case -1:
    case YM_CAN: {
      return;
    }
    default:
      break;
    }
  }
  
  int32_t ch;
  do {
    __ym_putchar(YM_EOT);
    ch = __ym_getchar(timeout_ms);
  } while ((ch != YM_ACK) && (ch != -1));
  
  /* send last data packet */
  if (ch == YM_ACK) {
    ch = __ym_getchar(timeout_ms);
    if (ch == YM_CRC) {
      do {
        ym_send_packet0(0, 0);
        ch = __ym_getchar(timeout_ms);
      } while ((ch != YM_ACK) && (ch != -1));
    }
  }
}

/* ------------------------------------------------------- */
int32_t fymodem_send(int fd, uint8_t* txdata, size_t txsize, const char* filename)
{
  global_fd = fd;

  /* flush the RX FIFO, after a cool off delay */
  __ym_sleep_ms(1000);
  __ym_flush();
  (void)__ym_getchar(1000);

  /* not in the specs, send CRC here just for balance */
  int32_t ch;

  bool crc_nak = true;
  bool file_done = false;
  do {
    ym_send_packet0(filename, txsize);
    /* When the receiving program receives this block and successfully
       opened the output file, it shall acknowledge this block with an ACK
       character and then proceed with a normal XMODEM file transfer
       beginning with a "C" or NAK tranmsitted by the receiver. */
    ch = __ym_getchar(YM_PACKET_RX_TIMEOUT_MS);
    if (ch == YM_ACK) {
      ch = __ym_getchar(YM_PACKET_RX_TIMEOUT_MS);
      if (ch == YM_CRC) {
        ym_send_data_packets(txdata, txsize, YM_PACKET_RX_TIMEOUT_MS);
        /* success */
        file_done = true;
      }
    }
    else if ((ch == YM_CRC) && (crc_nak)) {
      crc_nak = false;
      continue;
    }
    else if ((ch != YM_NAK) || (crc_nak)) {
      goto tx_err_handler;
    }
  } while (! file_done);

  return txsize;

 tx_err_handler:
  printf("TX Error!\n");
  __ym_putchar(YM_CAN);
  __ym_putchar(YM_CAN);
  __ym_sleep_ms(1000);
  return 0;
}
