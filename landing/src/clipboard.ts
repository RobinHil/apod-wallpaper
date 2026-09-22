/**
 * Putting text on the clipboard, through whichever API the browser allows.
 *
 * The async clipboard is refused outside a secure context, by a permission
 * policy, and by anything that hands the page a `navigator.clipboard` that
 * throws, which is what an extension does when it decides a page has no
 * business writing there. `execCommand` is deprecated and still implemented
 * everywhere, and it asks nobody, so it is worth trying before giving up.
 */
export async function writeClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return writeThroughField(text);
  }
}

/**
 * The older way: a field holding the text, selected, copied, taken back out.
 *
 * The field is pushed off-screen rather than hidden, because what is not
 * rendered cannot be selected, and a field left in view would scroll the page
 * to itself the moment it takes the selection.
 */
function writeThroughField(text: string): boolean {
  const field = document.createElement("textarea");
  field.value = text;
  field.readOnly = true;
  field.style.cssText = "position:fixed;top:0;left:-9999px;opacity:0";
  document.body.append(field);
  field.select();

  let done = false;
  try {
    done = document.execCommand("copy");
  } catch {
    done = false;
  }

  field.remove();
  return done;
}

/**
 * Selects what an element says, so the keyboard can finish what a button
 * could not.
 */
export function selectContents(node: HTMLElement | null): boolean {
  const selection = document.getSelection();
  if (!node || !selection) return false;

  const range = document.createRange();
  range.selectNodeContents(node);
  selection.removeAllRanges();
  selection.addRange(range);
  return true;
}
