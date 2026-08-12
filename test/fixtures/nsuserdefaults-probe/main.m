// What terminal-notifier SEES when it reads an option value, and nothing else.
// It reads its options off NSUserDefaults' argument domain, so `-message (x`
// is handed to the old-style property list parser before any of its own code
// runs, and a value that fails to parse as a string arrives nil.
//
// The leading-backslash unescape below is REPLICATED, not exercised. Upstream
// documents it in `terminal-notifier -help` (2.0.0): "Note that in some
// circumstances the first character of a message has to be escaped in order to
// be recognized. An example of this is when using an open bracket, which has
// to be escaped like so: '\['", implemented there as a
// `SubscriptAndUnescape` category. If upstream's rule ever drifts from this
// one line, the drill catches it, not this probe.
//
// Prints the parsed title then message, NUL separated, or the token
// NOT-A-STRING for a value the parser did not yield a string for.
#import <Foundation/Foundation.h>

int main(void) {
  @autoreleasepool {
    NSUserDefaults *defaults = [NSUserDefaults standardUserDefaults];
    for (NSString *key in @[ @"title", @"message" ]) {
      NSString *value = [defaults stringForKey:key];
      if ([value hasPrefix:@"\\"]) {
        value = [value substringFromIndex:1];
      }
      const char *out = value ? [value UTF8String] : "NOT-A-STRING";
      fwrite(out, 1, strlen(out), stdout);
      fputc('\0', stdout);
    }
  }
  return 0;
}
