// Tauri 1.x API Type Declarations
interface Window {
  __tauri__?: {
    dialog?: {
      open?: (options: {
        directory?: boolean;
        multiple?: boolean;
        title?: string;
      }) => Promise<string | string[] | null>;
    };
    fs?: {
      readDir?: (path: string) => Promise<Array<{
        name: string;
        path: string;
        type: 'file' | 'directory';
      }>>;
      readTextFile?: (path: string) => Promise<string>;
    };
  };
}