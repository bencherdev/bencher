export const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

export const parentPath = (pathname) => {
  return `${pathname.substr(0, pathname.lastIndexOf("/"))}`;
};

export const addPath = (pathname) => {
  return `${pathname}/add`;
};

export const viewPath = (pathname, datum) => {
  return `${pathname}/${datum?.slug}`;
};
