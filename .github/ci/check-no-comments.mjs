import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

const here = path.dirname(fileURLToPath(import.meta.url))
const sharedAllowList = path.join(here, 'comment-allow-list.txt')
const projectAllowList = '.wexo/comment-allow-list.txt'
const projectVendoredList = '.wexo/comment-vendored-files.txt'

const STRINGS = /"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`/g
const SLASH_MARKER = /(?<!\\)(?:(?<!:)[/][/]|[/][*])/
const BLOCK_OPENER = /(?<!\\)[/][*]/g
const HASH_MARKER = /(?<=^|[\s;&|(])#/
const PHP_HASH_MARKER = /(?<=^|[\s;&|(])#(?!\[)/
const BLADE_MARKER = /\{\{--/
const BLADE_CLOSER = /^\s*--\}\}/
const HUNK = /^@@ -\S+ \+(\d+)/

function closingIndex(line, from, quote) {
  for (let i = from; i < line.length; i++) {
    if (line[i] === '\\') i++
    else if (line[i] === quote) return i
  }
  return -1
}

const TEMPLATE_TOKEN = /"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`|\/\/|`/g

function openingBacktick(text) {
  const stop = Array.from(text.matchAll(TEMPLATE_TOKEN)).find((match) => match[0] === '//' || match[0] === '`')
  return stop?.[0] === '`' ? stop.index : -1
}

export function blankTemplateText(line, inString) {
  const closed = inString ? closingIndex(line, 0, '`') : -1
  if (inString && closed < 0) return { text: ' '.repeat(line.length), inString: true }
  const head = inString ? ' '.repeat(closed + 1) : ''
  const body = line.slice(inString ? closed + 1 : 0)
  const opened = openingBacktick(body)
  if (opened < 0) return { text: head + body, inString: false }
  return { text: head + body.slice(0, opened) + ' '.repeat(body.length - opened), inString: true }
}

const HTML = 'X'
const PHP_OPEN = /<\?(?:php\b|=)/
const HEREDOC_OPEN = /<<<[ \t]*(['"]?)([A-Za-z_]\w*)\1[ \t]*$/

export function blankPhpText(line, state) {
  const open = typeof state === 'string' ? state : null
  let out = ''
  let i = 0
  let quote = open && !open.startsWith('H:') && open !== HTML ? open : null
  if (open === HTML) {
    const back = PHP_OPEN.exec(line)
    if (!back) return { text: ' '.repeat(line.length), inString: HTML }
    out = ' '.repeat(back.index + back[0].length)
    i = back.index + back[0].length
  }
  if (open?.startsWith('H:')) {
    const id = open.slice(2)
    const end = new RegExp(`^(\\s*)${id}\\b`).exec(line)
    if (!end) return { text: ' '.repeat(line.length), inString: open }
    out = ' '.repeat(end[0].length)
    i = end[0].length
  }
  while (i < line.length) {
    const ch = line[i]
    if (quote) {
      const closed = closingIndex(line, i, quote)
      if (closed < 0) return { text: out + ' '.repeat(line.length - i), inString: quote }
      out += ' '.repeat(closed + 1 - i)
      i = closed + 1
      quote = null
    } else if (ch === '"' || ch === "'") {
      const closed = closingIndex(line, i + 1, ch)
      if (closed < 0) return { text: out + line.slice(i, i + 1) + ' '.repeat(line.length - i - 1), inString: ch }
      out += line.slice(i, closed + 1)
      i = closed + 1
    } else if (line.startsWith('?>', i)) {
      const back = PHP_OPEN.exec(line.slice(i))
      if (!back) return { text: out + line.slice(i, i + 2) + ' '.repeat(line.length - i - 2), inString: HTML }
      const resume = i + back.index + back[0].length
      out += line.slice(i, i + 2) + ' '.repeat(resume - i - 2)
      i = resume
    } else if (line.startsWith('//', i) || line.startsWith('/*', i) || (ch === '#' && line[i + 1] !== '[')) {
      return { text: out + line.slice(i), inString: false }
    } else {
      const heredoc = HEREDOC_OPEN.exec(line.slice(i))
      if (heredoc && line.startsWith('<<<', i)) return { text: out + ' '.repeat(line.length - i), inString: `H:${heredoc[2]}` }
      out += ch
      i++
    }
  }
  return { text: out, inString: false }
}

function readEntries(file) {
  return readFileSync(file, 'utf8')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
}

export function loadAllowList(extra = projectAllowList) {
  const project = extra && existsSync(extra) ? readEntries(extra) : []
  return [...readEntries(sharedAllowList), ...project]
}

export function loadVendored(file = projectVendoredList) {
  return file && existsSync(file) ? readEntries(file) : []
}

function firstComment(line, marker) {
  const masked = line.replace(STRINGS, (quoted) => ' '.repeat(quoted.length))
  const index = masked.search(marker)
  return index >= 0 ? line.slice(index) : null
}

function slashComment(line, inBlock) {
  const continuation = inBlock ? line.trim() : null
  return firstComment(line, SLASH_MARKER) ?? continuation
}

function hashComment(line) {
  return firstComment(line, HASH_MARKER)
}

function phpComment(line, inBlock) {
  return slashComment(line, inBlock) ?? firstComment(line, PHP_HASH_MARKER)
}

function bladeComment(line) {
  return firstComment(line, BLADE_MARKER) ?? (BLADE_CLOSER.test(line) ? line.trim() : null)
}

const CHECKERS = [
  { files: /\.blade\.php$/, find: bladeComment },
  { files: /\.php$/, find: phpComment, blank: blankPhpText, start: HTML, hashLineComments: true },
  { files: /\.(ts|tsx|js|jsx|mjs|cjs)$/, find: slashComment, blank: blankTemplateText },
  { files: /\.(sh|bash)$/, find: hashComment },
  { files: /\.ya?ml$/, find: hashComment },
]

function checkerOf(file) {
  return CHECKERS.find((checker) => checker.files.test(file))
}

export function commentOf(file, line, inBlock = false) {
  return checkerOf(file)?.find(line, inBlock) ?? null
}

function blockOpenAfter(line, inBlock) {
  const code = line.replace(STRINGS, (quoted) => ' '.repeat(quoted.length))
  const opened = Math.max(-1, ...Array.from(code.matchAll(BLOCK_OPENER), (match) => match.index))
  const closed = code.lastIndexOf('*/')
  if (opened > closed) return true
  return inBlock && closed < 0
}

function isDirective(comment, allowList) {
  const body = comment.replace(/^(\/\/+|\/\*+|#|\{\{--|\*)\s*/, '')
  if (/^\/?$/.test(body)) return true
  return allowList.some((entry) => comment.startsWith(entry) || body.startsWith(entry))
}

function headerFile(raw) {
  return raw === '+++ /dev/null' ? null : raw.slice(4).replace(/\t$/, '').replace(/^"(.*)"$/, '$1').replace(/^[a-z][/]/, '')
}

function codeBeforeLineComment(file, text) {
  const masked = text.replace(STRINGS, (quoted) => ' '.repeat(quoted.length))
  const slashes = masked.search(/(?<!\\)(?<!:)[/][/]/)
  const hashes = checkerOf(file)?.hashLineComments ? masked.search(PHP_HASH_MARKER) : -1
  const line = [slashes, hashes].filter((index) => index >= 0).sort((a, b) => a - b)[0] ?? -1
  const block = masked.search(BLOCK_OPENER)
  return line >= 0 && (block < 0 || line < block) ? text.slice(0, line) : text
}

function blankStrings(file, text, start) {
  if (start.inBlock) return { text, inString: start.inString }
  const blank = checkerOf(file)?.blank
  return blank ? blank(text, start.inString) : { text, inString: start.inString }
}

function scanLine(file, text, start) {
  const blanked = blankStrings(file, text, start)
  const code = start.inBlock ? blanked.text : codeBeforeLineComment(file, blanked.text)
  return { view: blanked.text, inBlock: blockOpenAfter(code, start.inBlock), inString: blanked.inString }
}

function startStates(file, lines) {
  const states = []
  let state = { inBlock: false, inString: checkerOf(file)?.start ?? false }
  for (const line of lines) {
    states.push(state)
    const { inBlock, inString } = scanLine(file, line, state)
    state = { inBlock, inString }
  }
  return states
}

function onHeader(state, raw) {
  if (!(raw.startsWith('+++ ') && state.previous.startsWith('--- '))) return null
  const file = headerFile(raw)
  const lines = file ? state.readLines(file) : null
  return { file, inBlock: false, inString: false, states: lines ? startStates(file, lines) : null }
}

function onHunk(state, raw) {
  const hunk = HUNK.exec(raw)
  return hunk ? { lineNo: Number(hunk[1]), inBlock: false, inString: false } : null
}

function onAdded(state, raw) {
  if (!state.file || !raw.startsWith('+')) return null
  const text = raw.slice(1)
  const start = state.states?.[state.lineNo - 1] ?? { inBlock: state.inBlock, inString: state.inString }
  const scanned = scanLine(state.file, text, start)
  return {
    lineNo: state.lineNo + 1,
    inBlock: scanned.inBlock,
    inString: scanned.inString,
    added: { file: state.file, line: state.lineNo, text, view: scanned.view, inBlock: start.inBlock },
  }
}

function advance(state, raw) {
  const change = onHeader(state, raw) ?? onHunk(state, raw) ?? onAdded(state, raw)
  return { ...state, added: null, ...change, previous: raw }
}

function violationOf(added, allowList) {
  if (!added) return null
  const comment = commentOf(added.file, added.view, added.inBlock)
  const allowed = comment === null || isDirective(comment, allowList)
  return allowed ? null : { file: added.file, line: added.line, text: added.text.trim() }
}

export function findViolations(diff, allowList, readLines = () => null) {
  const violations = []
  let state = { file: null, lineNo: 0, previous: '', added: null, inBlock: false, inString: false, states: null, readLines }
  for (const raw of diff.split('\n')) {
    state = advance(state, raw)
    const violation = violationOf(state.added, allowList)
    if (violation) violations.push(violation)
  }
  return violations
}

const TREE_SCOPE = process.env.MERGE_CHECKS_COMMENTS_SCOPE === 'tree'

export const FAILURE_MESSAGE = [
  `${TREE_SCOPE ? 'The tree at the merge request head contains' : 'This merge request adds'} code comments (.php, .blade.php, .ts, .tsx, .js, .jsx, .mjs, .cjs, .sh, .bash, .yml, .yaml).`,
  'Text inside shell heredocs and YAML block scalars is not tracked: a line there that holds a comment marker is flagged on its own (it opens no block). Move that text to a file, or skip the path with MERGE_CHECKS_COMMENTS_EXCLUDE.',
  'Put the reason where it can be checked instead: in a name, a type, a test, or a decision record.',
  `Tool directives (shebang, @ts-expect-error, shellcheck disable, @phpstan-) are allowed; a project adds its own, one per line, in ${projectAllowList}.`,
  `Vendored files (copied from elsewhere, one path per line) are skipped when listed in ${projectVendoredList}.`,
].join('\n')

function readLinesAt(rev) {
  return (file) => {
    try {
      return execFileSync('git', ['show', `${rev}:${file}`], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'], maxBuffer: 64 * 1024 * 1024 }).split('\n')
    } catch {
      return null
    }
  }
}

function main() {
  const diff = readFileSync(process.argv[2] ?? 0, 'utf8')
  const excluded = new RegExp(process.env.MERGE_CHECKS_COMMENTS_EXCLUDE || '^$')
  const vendored = loadVendored()
  const violations = findViolations(diff, loadAllowList(), readLinesAt(process.env.CI_MERGE_REQUEST_SOURCE_BRANCH_SHA || 'HEAD')).filter((v) => !excluded.test(v.file) && !vendored.includes(v.file))
  for (const v of violations) console.error(`${v.file}:${v.line}: ${v.text}`)
  if (violations.length > 0) console.error(`\n${FAILURE_MESSAGE}`)
  else console.log('No added comments.')
  process.exitCode = violations.length > 0 ? 1 : 0
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main()
