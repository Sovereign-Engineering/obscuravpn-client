import commonClasses from '../common/common.module.css';
import Wordmark from '../res/obscura-wordmark.svg?react';

export default function ObscuraWordmark() {
  return <Wordmark className={commonClasses.wordmark} width={150} height='auto' />;
}
