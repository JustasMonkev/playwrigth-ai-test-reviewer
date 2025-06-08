declare namespace React {
  type ReactNode = any;
  type ReactElement = any;
  interface FC<P = object> {
    (props: P): ReactElement | null;
  }
}

declare namespace JSX {
  interface IntrinsicElements {
    [elemName: string]: any;
  }
  
  interface Element {
    type: any;
    props: any;
    key: any;
  }
}
