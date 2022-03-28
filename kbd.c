#include <asm-generic/int-ll64.h>
#include <fcntl.h>
#include <linux/input.h>
#include <linux/kd.h>
#include <linux/keyboard.h>
#include <signal.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <termios.h>
#include <unistd.h>

// #define DEBUG

#define BITS_PER_LONG (sizeof(long) * 8)
#define NBITS(x) ((((x)-1) / BITS_PER_LONG) + 1)

void block_stdin() {
  struct termios stdin_term;
  tcgetattr(0, &stdin_term);
  stdin_term.c_lflag &= ~(ECHO | ICANON);
  stdin_term.c_cc[VMIN] = 0;
  stdin_term.c_cc[VTIME] = 1;
  tcsetattr(0, TCSAFLUSH, &stdin_term);
}

int main() {
  block_stdin();
  int FileDevice = open("/dev/input/event4", O_RDONLY);
  if (FileDevice == -1) {
    perror("Failed to open device");
    return 1;
  }
  int version;
  //----- GET DEVICE VERSION -----
  if (ioctl(FileDevice, EVIOCGVERSION, &version)) {
    perror("KeyboardMonitor can't get version");
    close(FileDevice);
    return 1;
  }
  fprintf(stderr, "Input driver version is %d.%d.%d\n", version >> 16,
          (version >> 8) & 0xff, version & 0xff);

  u_int16_t id[4];
  //----- GET DEVICE INFO -----
  ioctl(FileDevice, EVIOCGID, id);
  fprintf(stderr, "Input device ID: bus 0x%x vendor 0x%x product 0x%x version 0x%x\n",
  id[ID_BUS], id[ID_VENDOR], id[ID_PRODUCT], id[ID_VERSION]);

  unsigned long bit[EV_MAX][NBITS(KEY_MAX)];
  memset(bit, 0, sizeof(bit));
  ioctl(FileDevice, EVIOCGBIT(0, EV_MAX), bit[0]);

  int ReadDevice;
  struct input_event InputEvent[64];
  //----- READ KEYBOARD EVENTS -----
  while (1) {
    ReadDevice = read(FileDevice, InputEvent, sizeof(struct input_event) * 64);

    if (ReadDevice < (int)sizeof(struct input_event)) {
      // This should never happen
      perror("KeyboardMonitor error reading - keyboard lost?");
      close(FileDevice);
      return 1;
    } else {
      for (int Index = 0; Index < ReadDevice / sizeof(struct input_event);
           Index++) {
        // We have:
        //	InputEvent[Index].time		timeval: 16 bytes (8 bytes for
        // seconds, 8 bytes for microseconds)
        //	InputEvent[Index].type		See input-event-codes.h
        //	InputEvent[Index].code		See input-event-codes.h
        //	InputEvent[Index].value		01 for keypress, 00 for release,
        // 02 for autorepeat

        if (InputEvent[Index].type == EV_KEY) {
          if (InputEvent[Index].value == 2) {
            continue;
            // This is an auto repeat of a held down key
          }
          __u8 value = InputEvent[Index].value;
          __u16 code = InputEvent[Index].code;
          #ifdef DEBUG
          printf("key with code %d was %s\n", (int)code, value == 1 ? "pressed" : "released");
          #else
          write(1, &value, sizeof(__u8));
          write(1, &code, sizeof(__u16));
          #endif
          //----- KEY DOWN -----
        }
      }
    }
  }

  return 0;
}

//// input-event-codes.h

// ESC - 1
// 1-0 - 2-11
// BCKSP - 14
// TAB - 15
// q-] - 16-27
// \   - 43
// ~   - 41
// a'  - 30-40
// LINEFEED - 28
// LSHIFT - 42
// z-/ - 44-53
// RSHIFT - 54
// LCTRL - 29
// WIN - 125
// LALT - 56
// SPACE - 57
// MENU - 127
// DEL - 111
// PGUP - 104
// PGDN - 109
// UP - 103
// DOWN - 108
// RIGHT - 106
// LEFT - 105
// VOLUP - 115
// VOLDN - 114
// HOME - 102
// END - 107
// CAPSLK - 58
// F1-F10 - 59-68
// F11 - 87
// F12 - 88

// int fd = -1, oldkbmode = K_RAW;
// struct termios orig_kb;

// void clean_up(void) {
//   ioctl(fd, KDSKBMODE, oldkbmode);
//   tcsetattr(fd, 0, &orig_kb);
//   close(fd);
// }

// void block_stdin() {
//   struct termios stdin_term;
//   tcgetattr(0, &stdin_term);
//   stdin_term.c_lflag &= ~(ECHO | ICANON);
//   stdin_term.c_cc[VMIN] = 0;
//   stdin_term.c_cc[VTIME] = 1;
//   tcsetattr(0, TCSAFLUSH, &stdin_term);
// }

// void key_action(int keycode, bool pressed) {
//   // write(1, &pressed, sizeof(bool));
//   // write(1, &keycode, sizeof(int));
//   printf("Key with code: '%d' was %s\n", keycode, pressed ? "pressed" :
//   "released");
// }

// int main() {
//   block_stdin();
//   int keycode, pressed, controlPressed = 0;
//   unsigned char buf[18]; /* divisible by 3 */
//   int i, n;
//   struct termios newkbd;

//   /* Open and configure the keyboard */
//   fd = open("/dev/tty0", O_RDONLY);
//   if (fd == -1) {
//     perror("Failed to open keyboard device");
//     return 1;
//   }
//   tcgetattr(fd, &orig_kb);
//   tcgetattr(fd, &newkbd);
//   newkbd.c_lflag &= ~(ECHO | ICANON | ISIG);
//   newkbd.c_iflag = 0;
//   newkbd.c_cc[VMIN] = 18;
//   newkbd.c_cc[VTIME] = 1;
//   tcsetattr(fd, TCSAFLUSH, &newkbd);

//   /* Set medium raw mode: we receive keycodes. */
//   ioctl(fd, KDGKBMODE, &oldkbmode);
//   ioctl(fd, KDSKBMODE, K_MEDIUMRAW);

//   /* Restore the normal keyboard mode on exit */
//   atexit(clean_up);

//   while (1) {
//     /* Wait until a key is pressed or released */
//     n = read(fd, buf, sizeof(buf));

//     /* Retrieve the key code and whether the key was pressed or
//     released */
//     i = 0;
//     while (i < n) {
//       pressed = (buf[i] & 0x80) == 0x80 ? 0 : 1;
//       if (i + 2 < n && (buf[i] & 0x7f) == 0 && (buf[i + 1] & 0x80) != 0 &&
//           (buf[i + 2] & 0x80) != 0) {
//         keycode = ((buf[i + 1] & 0x7f) << 7) | (buf[i + 2] & 0x7f);
//         i += 3;
//       } else
//         keycode = (buf[i++] & 0x7f);
//     }
//     key_action(keycode, pressed);
//     /* Take appropriate action */
//     switch (keycode) {
//     case 29: // CTRL
//       controlPressed = pressed;
//       break;
//     case 46: // C
//       if (controlPressed && pressed)
//         exit(0);
//       break;
//     }
//   }
//   return 0;
// }
