// Normal imports
import { readFile } from 'fs/promises';

export async function loadData(path: string) {
  return readFile(path, 'utf-8');
}
